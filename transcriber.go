// Push-to-dictate for i3 (bindcode 105: hold Right Ctrl to record, release to paste).
// Build: CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o ~/.local/bin/transcriber transcriber.go
package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"mime/multipart"
	"net/http"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"
)

const (
	defaultAPIBase    = "https://api.groq.com/openai/v1"
	defaultAudioModel = "whisper-large-v3-turbo"
	defaultTextModel  = "qwen/qwen3.8-27b"
	defaultLanguage   = "en"

	// Per-stage latency budget after key release.
	transcribeTimeout = 15 * time.Second
	condenseTimeout   = 4 * time.Second
	pasteTimeout      = 3 * time.Second

	// Only condense transcripts longer than this. Below it, the LLM round-trip
	// costs more latency than it saves, and short dictations are usually
	// already in final form.
	condenseMinChars = 100

	// Pre-roll kept while idle so the first syllable isn't lost between
	// key-press and arecord spin-up.
	preRollDuration = 500 * time.Millisecond

	sampleRate     = 16000
	bytesPerSample = 2

	// Hard cap on a single recording to bound memory.
	maxRecordingDuration = 5 * time.Minute

	maxErrorBody = 4 << 10
)

var (
	preRollBytes      = int(preRollDuration.Seconds() * sampleRate * bytesPerSample)
	maxRecordingBytes = int(maxRecordingDuration.Seconds() * sampleRate * bytesPerSample)
)

// ---------- capture ----------

// capture runs a single long-lived arecord process and keeps a short rolling
// pre-roll buffer. beginRecording snapshots the pre-roll plus everything that
// follows; endRecording hands off the recording as raw PCM.
type capture struct {
	mu       sync.Mutex
	cmd      *exec.Cmd
	done     chan struct{}
	stopping bool

	preBuf    []byte
	recBuf    []byte
	recording bool
	truncated bool
}

func newCapture() *capture { return &capture{} }

func (c *capture) start() error {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.cmd != nil {
		return nil
	}

	cmd := exec.Command("arecord",
		"-q",
		"-f", "S16_LE",
		"-r", fmt.Sprint(sampleRate),
		"-c", "1",
		"-t", "raw",
		"-",
	)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("arecord stdout: %w", err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("start arecord: %w", err)
	}

	done := make(chan struct{})
	c.cmd = cmd
	c.done = done

	go func() {
		c.readLoop(stdout)
		err := cmd.Wait()

		c.mu.Lock()
		stopping := c.stopping
		if c.cmd == cmd {
			c.cmd = nil
			c.done = nil
		}
		c.mu.Unlock()
		close(done)

		if err != nil && !stopping {
			log.Printf("arecord exited: %v; restarting", err)
			time.Sleep(200 * time.Millisecond)
			if err := c.start(); err != nil {
				log.Printf("restart arecord: %v", err)
			}
		}
	}()
	return nil
}

func (c *capture) readLoop(r io.Reader) {
	buf := make([]byte, 8192)
	for {
		n, err := r.Read(buf)
		if n > 0 {
			chunk := buf[:n]
			c.mu.Lock()
			if c.recording {
				if len(c.recBuf) < maxRecordingBytes {
					c.recBuf = append(c.recBuf, chunk...)
				} else if !c.truncated {
					c.truncated = true
					log.Printf("recording truncated at %v", maxRecordingDuration)
				}
			}
			c.preBuf = append(c.preBuf, chunk...)
			if len(c.preBuf) > preRollBytes {
				drop := len(c.preBuf) - preRollBytes
				c.preBuf = c.preBuf[drop:]
				if cap(c.preBuf) > 4*preRollBytes {
					c.preBuf = append([]byte(nil), c.preBuf...)
				}
			}
			c.mu.Unlock()
		}
		if err != nil {
			return
		}
	}
}

func (c *capture) beginRecording() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.recBuf = append(c.recBuf[:0], c.preBuf...)
	c.recording = true
	c.truncated = false
}

func (c *capture) endRecording() []byte {
	c.mu.Lock()
	defer c.mu.Unlock()
	if !c.recording {
		return nil
	}
	c.recording = false
	out := c.recBuf
	c.recBuf = nil
	return out
}

func (c *capture) kill() {
	c.mu.Lock()
	c.stopping = true
	cmd := c.cmd
	done := c.done
	c.mu.Unlock()

	if cmd != nil && cmd.Process != nil {
		_ = cmd.Process.Signal(syscall.SIGTERM)
	}
	if done != nil {
		select {
		case <-done:
		case <-time.After(500 * time.Millisecond):
			if cmd != nil && cmd.Process != nil {
				_ = cmd.Process.Kill()
			}
			<-done
		}
	}
}

// ---------- WAV ----------

func wavHeader(pcmLen int) []byte {
	h := make([]byte, 44)
	copy(h[0:], "RIFF")
	binary.LittleEndian.PutUint32(h[4:], uint32(36+pcmLen))
	copy(h[8:], "WAVE")
	copy(h[12:], "fmt ")
	binary.LittleEndian.PutUint32(h[16:], 16)                        // fmt chunk size
	binary.LittleEndian.PutUint16(h[20:], 1)                         // PCM
	binary.LittleEndian.PutUint16(h[22:], 1)                         // mono
	binary.LittleEndian.PutUint32(h[24:], sampleRate)                // sample rate
	binary.LittleEndian.PutUint32(h[28:], sampleRate*bytesPerSample) // byte rate
	binary.LittleEndian.PutUint16(h[32:], bytesPerSample)            // block align
	binary.LittleEndian.PutUint16(h[34:], 8*bytesPerSample)          // bits per sample
	copy(h[36:], "data")
	binary.LittleEndian.PutUint32(h[40:], uint32(pcmLen))
	return h
}

// ---------- Groq client ----------

type client struct {
	apiKey string
	http   *http.Client
}

func newClient(apiKey string) *client {
	return &client{
		apiKey: apiKey,
		http: &http.Client{
			// Per-request deadlines come from context.
			Transport: &http.Transport{
				MaxIdleConns:        4,
				MaxIdleConnsPerHost: 4,
				IdleConnTimeout:     90 * time.Second,
			},
		},
	}
}

func (c *client) transcribe(ctx context.Context, wav []byte) (string, error) {
	var body bytes.Buffer
	w := multipart.NewWriter(&body)
	_ = w.WriteField("model", env("TRANSCRIBER_AUDIO_MODEL", defaultAudioModel))
	_ = w.WriteField("language", env("TRANSCRIBER_LANGUAGE", defaultLanguage))
	fw, err := w.CreateFormFile("file", "a.wav")
	if err != nil {
		return "", err
	}
	if _, err := fw.Write(wav); err != nil {
		return "", err
	}
	if err := w.Close(); err != nil {
		return "", err
	}

	var v struct {
		Text string `json:"text"`
	}
	if err := c.post(ctx, "/audio/transcriptions", w.FormDataContentType(), &body, &v); err != nil {
		return "", err
	}
	return strings.TrimSpace(v.Text), nil
}

func (c *client) condense(ctx context.Context, text string) (string, error) {
	payload, err := json.Marshal(map[string]any{
		"model": env("TRANSCRIBER_TEXT_MODEL", defaultTextModel),
		"input": []map[string]string{{
			"role":    "user",
			"content": "Condense the following text: " + text,
		}},
		"max_output_tokens": 1024,
		"reasoning":         map[string]string{"effort": "none"},
	})
	if err != nil {
		return "", err
	}

	var v struct {
		Output []struct {
			Type    string `json:"type"`
			Content []struct {
				Type string `json:"type"`
				Text string `json:"text"`
			} `json:"content"`
		} `json:"output"`
	}
	if err := c.post(ctx, "/responses", "application/json", bytes.NewReader(payload), &v); err != nil {
		return "", err
	}
	var sb strings.Builder
	for _, item := range v.Output {
		if item.Type != "message" {
			continue
		}
		for _, part := range item.Content {
			if part.Type != "output_text" {
				continue
			}
			sb.WriteString(part.Text)
		}
	}
	if sb.Len() == 0 {
		return "", errors.New("no output text in response")
	}
	return strings.TrimSpace(sb.String()), nil
}

func (c *client) post(ctx context.Context, path, contentType string, body io.Reader, out any) error {
	base := env("TRANSCRIBER_API_BASE", defaultAPIBase)
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, base+path, body)
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", contentType)
	req.Header.Set("Authorization", "Bearer "+c.apiKey)

	resp, err := c.http.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		b, _ := io.ReadAll(io.LimitReader(resp.Body, maxErrorBody))
		return fmt.Errorf("%s: http %d: %s", path, resp.StatusCode, strings.TrimSpace(string(b)))
	}
	return json.NewDecoder(resp.Body).Decode(out)
}

// ---------- paste ----------

func paste(ctx context.Context, s string) error {
	clip := exec.CommandContext(ctx, "xclip", "-selection", "clipboard", "-i")
	clip.Stdin = strings.NewReader(s)
	if err := clip.Run(); err != nil {
		return fmt.Errorf("xclip: %w", err)
	}
	// xclip forks to serve the selection; give it a moment to take ownership
	// before we synthesize ctrl+v, otherwise the paste can race.
	select {
	case <-time.After(30 * time.Millisecond):
	case <-ctx.Done():
		return ctx.Err()
	}
	key := exec.CommandContext(ctx, "xdotool", "key", "--clearmodifiers", "ctrl+v")
	if err := key.Run(); err != nil {
		return fmt.Errorf("xdotool: %w", err)
	}
	return nil
}

// ---------- pipeline ----------

func handleUtterance(parent context.Context, c *client, pcm []byte) {
	// Drop sub-100ms blips (e.g. accidental taps of the key).
	if len(pcm) < sampleRate*bytesPerSample/10 {
		return
	}
	// Detach from the daemon context so a shutdown doesn't abort an in-flight
	// paste, but keep a hard bound on every stage so we can't hang forever.
	base := context.WithoutCancel(parent)

	t0 := time.Now()
	wav := append(wavHeader(len(pcm)), pcm...)

	tctx, tcancel := context.WithTimeout(base, transcribeTimeout)
	text, err := c.transcribe(tctx, wav)
	tcancel()
	if err != nil {
		log.Printf("transcribe: %v", err)
		return
	}
	if text == "" {
		log.Print("transcribe: empty transcript")
		return
	}

	out := text
	if len(text) >= condenseMinChars {
		cctx, ccancel := context.WithTimeout(base, condenseTimeout)
		cleaned, err := c.condense(cctx, text)
		ccancel()
		if err != nil {
			log.Printf("condense: %v (using raw transcript)", err)
		} else {
			out = cleaned
		}
	}

	pctx, pcancel := context.WithTimeout(base, pasteTimeout)
	defer pcancel()
	if err := paste(pctx, out); err != nil {
		log.Printf("paste: %v", err)
		return
	}

	log.Printf("%dB audio -> %d chars in %v", len(pcm), len(out), time.Since(t0))
}

// ---------- IPC ----------

func fifoPath() string {
	if dir := os.Getenv("XDG_RUNTIME_DIR"); dir != "" {
		return filepath.Join(dir, "transcriber.fifo")
	}
	dir := filepath.Join(os.TempDir(), fmt.Sprintf("transcriber-%d", os.Getuid()))
	return filepath.Join(dir, "fifo")
}

func openFIFO(path string) (*os.File, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, err
	}
	if err := syscall.Mkfifo(path, 0o600); err != nil && !errors.Is(err, os.ErrExist) {
		return nil, err
	}
	fi, err := os.Lstat(path)
	if err != nil {
		return nil, err
	}
	if fi.Mode()&os.ModeNamedPipe == 0 {
		return nil, fmt.Errorf("refusing to use %s: not a FIFO", path)
	}
	return os.OpenFile(path, os.O_RDWR|syscall.O_NOFOLLOW, 0)
}

func sendCommand(path, cmd string) error {
	fi, err := os.Lstat(path)
	if err != nil {
		return fmt.Errorf("stat fifo %s: %w (is the daemon running?)", path, err)
	}
	if fi.Mode()&os.ModeNamedPipe == 0 {
		return fmt.Errorf("refusing to write to %s: not a FIFO", path)
	}
	f, err := os.OpenFile(path, os.O_WRONLY|syscall.O_NONBLOCK|syscall.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("open fifo %s: %w (is the daemon running?)", path, err)
	}
	defer f.Close()
	if _, err := f.WriteString(cmd + "\n"); err != nil {
		return fmt.Errorf("write fifo: %w", err)
	}
	return nil
}

// ---------- main ----------

func main() {
	log.SetFlags(0)
	log.SetPrefix("transcriber: ")

	if len(os.Args) > 1 {
		if err := sendCommand(fifoPath(), os.Args[1]); err != nil {
			log.Fatal(err)
		}
		return
	}

	apiKey := os.Getenv("GROQ_API_KEY")
	if apiKey == "" {
		log.Fatal("GROQ_API_KEY is not set")
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	path := fifoPath()
	f, err := openFIFO(path)
	if err != nil {
		log.Fatalf("open fifo %s: %v", path, err)
	}
	defer func() {
		f.Close()
		_ = os.Remove(path)
	}()

	capt := newCapture()
	if err := capt.start(); err != nil {
		log.Fatalf("start capture: %v", err)
	}
	defer capt.kill()

	cli := newClient(apiKey)

	go func() {
		<-ctx.Done()
		f.Close()
	}()

	log.Printf("listening on %s (audio=%s text=%s condense_min=%d)",
		path,
		env("TRANSCRIBER_AUDIO_MODEL", defaultAudioModel),
		env("TRANSCRIBER_TEXT_MODEL", defaultTextModel),
		condenseMinChars,
	)

	r := bufio.NewReader(f)
	for {
		line, err := r.ReadString('\n')
		if err != nil {
			if ctx.Err() != nil {
				return
			}
			if errors.Is(err, io.EOF) {
				continue
			}
			log.Printf("read fifo: %v", err)
			return
		}

		switch strings.TrimSpace(line) {
		case "":
		case "start":
			capt.beginRecording()
		case "stop":
			go handleUtterance(ctx, cli, capt.endRecording())
		case "quit":
			return
		default:
			log.Printf("unknown command: %q", strings.TrimSpace(line))
		}
	}
}

func env(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

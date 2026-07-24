#!/usr/bin/env python3
"""Extract the Uzum access_token from Brave/Chrome cookies.
Handles v11 AES-GCM encrypted cookies using the system keyring.
"""
import base64
import os
import sqlite3
import subprocess
import sys

COOKIE_DBS = [
    os.path.expanduser("~/.config/BraveSoftware/Brave-Browser/Default/Cookies"),
    os.path.expanduser("~/.config/google-chrome/Default/Cookies"),
    os.path.expanduser("~/.config/chromium/Default/Cookies"),
]


def _get_key(browser_id: str) -> bytes | None:
    try:
        r = subprocess.run(
            ["secret-tool", "lookup", "application", browser_id],
            capture_output=True, text=True, timeout=5,
        )
        if r.returncode != 0 or not r.stdout.strip():
            return None
        return base64.b64decode(r.stdout.strip())
    except Exception:
        return None


def _try_decrypt(encrypted: bytes, key: bytes) -> str | None:
    if len(encrypted) < 4:
        return None

    try:
        from cryptography.hazmat.primitives.ciphers.aead import AESGCM
        from cryptography.hazmat.primitives import hashes
        from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
        from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

        version = encrypted[:3]
        if version == b"v11":
            nonce = encrypted[3:15]
            ct_tag = encrypted[15:]

            # Derive AES-256 key via PBKDF2-HMAC-SHA1
            kdf = PBKDF2HMAC(algorithm=hashes.SHA1(), length=32, salt=b"saltysalt", iterations=1)
            aes_key = kdf.derive(key) if len(key) < 32 else key

            aesgcm = AESGCM(aes_key)
            for aad in (b"v11", b"v10", None, b""):
                try:
                    plain = aesgcm.decrypt(nonce, ct_tag, aad)
                    if plain and len(plain) > 10:
                        return plain.decode("utf-8")
                except Exception:
                    pass

            # Also try raw key as AES-128-GCM
            try:
                aesgcm = AESGCM(key)
                for aad in (b"v11", b"v10", None, b""):
                    try:
                        plain = aesgcm.decrypt(nonce, ct_tag, aad)
                        if plain and len(plain) > 10:
                            return plain.decode("utf-8")
                    except Exception:
                        pass
            except Exception:
                pass

            # Try AES-128-CBC (v10 fallback)
            try:
                iv = encrypted[3:19]
                ct_body = encrypted[19:-20]
                cipher = Cipher(algorithms.AES(key), modes.CBC(iv))
                dec = cipher.decryptor()
                padded = dec.update(ct_body) + dec.finalize()
                pad_len = padded[-1]
                if 1 <= pad_len <= 16:
                    plain = padded[:-pad_len]
                    if plain and len(plain) > 10:
                        return plain.decode("utf-8")
            except Exception:
                pass

        # Plaintext cookie
        try:
            return encrypted.decode("utf-8")
        except Exception:
            pass

    except ImportError:
        pass

    return None


def get_token() -> str | None:
    for db_path in COOKIE_DBS:
        if not os.path.isfile(db_path):
            continue

        try:
            conn = sqlite3.connect(db_path)
            cur = conn.cursor()
            cur.execute(
                "SELECT encrypted_value, value FROM cookies "
                "WHERE host_key='.uzum.uz' AND name='access_token'"
            )
            row = cur.fetchone()
            conn.close()
        except Exception:
            continue

        if row is None:
            continue

        enc_val, plain_val = row

        # Plaintext column first
        if plain_val and plain_val.startswith("eyJ"):
            return plain_val

        if not enc_val:
            continue

        encrypted = bytes(enc_val)

        # Plaintext in encrypted_value column
        try:
            text = encrypted.decode("utf-8")
            if text.startswith("eyJ"):
                return text
        except Exception:
            pass

        # Determine browser app id
        p = db_path
        key_name = "brave" if "Brave" in p else "chromium" if "chromium" in p else "chrome"

        key = _get_key(key_name)
        if key is None:
            continue

        result = _try_decrypt(encrypted, key)
        if result:
            return result

    return None


if __name__ == "__main__":
    token = get_token()
    if token:
        sys.stdout.write(token)
    else:
        sys.exit(1)

#!/bin/bash -e

for cmd in curl jq sudo; do 
  command -v "$cmd" >/dev/null || { echo "$cmd not found" >&2; exit 1; }
done

version="$(curl -s 'https://go.dev/dl/?mode=json' | jq -r '.[0].version')"
current="$(/usr/local/go/bin/go version 2>/dev/null | awk '{print $3}')"

if [[ "$current" == "$version" ]]; then
  echo "Go is already up-to-date at version ${version}"
  exit 0
fi

url="https://golang.org/dl/${version}.linux-amd64.tar.gz"
curl -sL "$url" | sudo tar -C /usr/local -xzf - --transform='s,^go,go.new,' && \
  sudo rm -rf /usr/local/go && \
  sudo mv /usr/local/go.new /usr/local/go && \
  echo "Go updated to version ${version}"

/usr/local/go/bin/go version

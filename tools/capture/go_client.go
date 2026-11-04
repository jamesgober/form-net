// Sends the client capture bodies with Go's mime/multipart; run by clients.py.
package main

import (
	"bytes"
	"mime/multipart"
	"net/http"
	"os"
)

type file struct {
	name, filename, ctype string
	data                  []byte
}

// filePart uses CreateFormFile, so the quoting and the Content-Type
// (application/octet-stream) are Go's own.
func filePart(w *multipart.Writer, f file) {
	p, err := w.CreateFormFile(f.name, f.filename)
	if err != nil {
		panic(err)
	}
	p.Write(f.data)
}

func send(url string, build func(w *multipart.Writer)) {
	var body bytes.Buffer
	w := multipart.NewWriter(&body)
	build(w)
	w.Close()
	resp, err := http.Post(url, w.FormDataContentType(), &body)
	if err != nil {
		panic(err)
	}
	resp.Body.Close()
}

func main() {
	base := os.Args[1]
	all := make([]byte, 256)
	for i := range all {
		all[i] = byte(i)
	}
	client := &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	http.DefaultClient = client
	send(base+"/capture/multipart", func(w *multipart.Writer) {
		w.WriteField("ascii", "hello world")
		w.WriteField("unicode", "héllo wörld ✓ 日本")
		w.WriteField("newline", "line1\r\nline2")
		filePart(w, file{"ascii_file", "hello.txt", "text/plain", []byte("plain text\n")})
		filePart(w, file{"unicode_file", "résumé ✓.txt", "text/plain", []byte("unicode name\n")})
		filePart(w, file{"multi", "a.txt", "text/plain", []byte("first\r\n--not a boundary\r\n")})
		filePart(w, file{"multi", "b.bin", "application/octet-stream", all})
	})
	send(base+"/capture/quote", func(w *multipart.Writer) {
		filePart(w, file{"quote_file", `na"me.txt`, "text/plain", []byte("quote\n")})
	})
}

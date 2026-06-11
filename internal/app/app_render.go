package app

import (
	"bytes"
	"encoding/base64"
	"image"
	"image/png"
	"sync"
	"time"

	"github.com/mantonx/nexus-open/internal/api"
)

// scaleNN2x returns a new RGBA image scaled 2× by nearest-neighbour replication.
// Each source pixel becomes a 2×2 block — correct for hardware pixel-art frames.
func scaleNN2x(src *image.RGBA) *image.RGBA {
	sw, sh := src.Bounds().Dx(), src.Bounds().Dy()
	dst := image.NewRGBA(image.Rect(0, 0, sw*2, sh*2))
	for y := 0; y < sh; y++ {
		for x := 0; x < sw; x++ {
			c := src.RGBAAt(x, y)
			dst.SetRGBA(x*2, y*2, c)
			dst.SetRGBA(x*2+1, y*2, c)
			dst.SetRGBA(x*2, y*2+1, c)
			dst.SetRGBA(x*2+1, y*2+1, c)
		}
	}
	return dst
}

// pngBufPool recycles the byte buffers used for PNG encoding the WS preview
// frames. The frame is scaled 2× before encoding (1280×96); pre-size to 16 KB.
var pngBufPool = sync.Pool{
	New: func() any {
		b := bytes.NewBuffer(make([]byte, 0, 16384))
		return b
	},
}

// pngEncoderPool implements png.EncoderBufferPool, recycling the internal
// flate writer and scratch buffers across encode calls.
type pngEncoderPool struct{ p sync.Pool }

func (p *pngEncoderPool) Get() *png.EncoderBuffer {
	return p.p.Get().(*png.EncoderBuffer)
}
func (p *pngEncoderPool) Put(b *png.EncoderBuffer) { p.p.Put(b) }

var sharedPNGPool = &pngEncoderPool{
	p: sync.Pool{New: func() any { return new(png.EncoderBuffer) }},
}

// renderLoop continuously renders frames and sends them to the device.
// Every 3rd frame (~10 FPS) is also broadcast to WebSocket clients as a base64 PNG.
func (a *App) renderLoop() {
	defer a.wg.Done()

	const targetFPS = 30
	frameDuration := time.Second / targetFPS
	ticker := time.NewTicker(frameDuration)
	defer ticker.Stop()

	a.logger.Info("render loop started", "fps", targetFPS)

	var frameCount int
	var lastSentPix []byte
	wsHub := a.apiServer.Hub()

	// Pool the flate compressor internals via EncoderBufferPool so the
	// Huffman tables and LZ77 scratch buffers are reused across frames.
	enc := &png.Encoder{CompressionLevel: png.BestSpeed, BufferPool: sharedPNGPool}

	for {
		select {
		case <-a.ctx.Done():
			a.logger.Info("render loop stopped")
			return

		case <-ticker.C:
			frame, err := a.zoneManager.RenderFrame()
			if err != nil {
				a.logger.Error("failed to render frame", "error", err)
				continue
			}

			// Send to device if connected and the frame has changed.
			// Skipping identical frames eliminates all 121 USB bulk transfers
			// per tick at idle — the dominant cost when content is static.
			if a.device.IsConnected() {
				if !bytes.Equal(frame.Pix, lastSentPix) {
					if err := a.device.SendFrame(a.ctx, frame.Pix); err != nil {
						a.logger.Debug("failed to send frame", "error", err)
					} else {
						if len(lastSentPix) != len(frame.Pix) {
							lastSentPix = make([]byte, len(frame.Pix))
						}
						copy(lastSentPix, frame.Pix)
					}
				}
			}

			// During transitions broadcast every frame (30fps) so the WS
			// analyser and Flutter preview see the full motion. Otherwise
			// subsample to every 3rd frame (~10fps) to keep bandwidth low.
			frameCount++
			if a.zoneManager.IsTransitioning() || frameCount%3 == 0 {
				buf := pngBufPool.Get().(*bytes.Buffer)
				buf.Reset()
				if err := enc.Encode(buf, scaleNN2x(frame)); err == nil {
					encoded := base64.StdEncoding.EncodeToString(buf.Bytes())
					wsHub.Broadcast(api.WSMessage{Type: "frame", Data: encoded})
				}
				pngBufPool.Put(buf)
			}
		}
	}
}

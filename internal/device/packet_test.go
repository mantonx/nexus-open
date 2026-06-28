package device

import (
	"testing"
)

// totalChunksFor returns the number of 1016-byte payload chunks needed for n bytes.
func totalChunksFor(n int) int {
	const maxPayload = 1024 - 8
	return (n + maxPayload - 1) / maxPayload
}

// A full 640×48 RGBA frame fits in exactly 121 chunks:
//   640 * 48 * 4 = 122880 bytes / 1016 = 121 (exact)
const wantChunks = 121

func TestBuildPacket_Size(t *testing.T) {
	rgba := make([]byte, FrameSize)
	pkt := buildPacket(rgba, 0, wantChunks)
	if len(pkt) != 1024 {
		t.Errorf("packet length = %d, want 1024", len(pkt))
	}
}

func TestBuildPacket_MagicHeader(t *testing.T) {
	rgba := make([]byte, FrameSize)
	pkt := buildPacket(rgba, 0, wantChunks)
	if pkt[0] != 0x02 || pkt[1] != 0x05 || pkt[2] != 0x40 {
		t.Errorf("magic bytes = %02x %02x %02x, want 02 05 40", pkt[0], pkt[1], pkt[2])
	}
}

func TestBuildPacket_FinalChunkFlag(t *testing.T) {
	rgba := make([]byte, FrameSize)
	total := totalChunksFor(FrameSize)

	for chunkNum := range total {
		pkt := buildPacket(rgba, chunkNum, total)
		isFinal := chunkNum == total-1
		if isFinal && pkt[3] != 0x01 {
			t.Errorf("chunk %d (final): flag byte = %02x, want 01", chunkNum, pkt[3])
		}
		if !isFinal && pkt[3] != 0x00 {
			t.Errorf("chunk %d (non-final): flag byte = %02x, want 00", chunkNum, pkt[3])
		}
	}
}

func TestBuildPacket_ChunkNumberEncoding(t *testing.T) {
	rgba := make([]byte, FrameSize)
	total := totalChunksFor(FrameSize)

	// Check a few chunk numbers including one that needs the high byte.
	cases := []int{0, 1, 63, 64, 120}
	for _, n := range cases {
		if n >= total {
			continue
		}
		pkt := buildPacket(rgba, n, total)
		gotLo, gotHi := int(pkt[4]), int(pkt[5])
		if gotLo != n&0xFF || gotHi != (n>>8)&0xFF {
			t.Errorf("chunk %d: number bytes = %02x %02x, want %02x %02x",
				n, gotLo, gotHi, n&0xFF, (n>>8)&0xFF)
		}
	}
}

func TestBuildPacket_PayloadLengthEncoding(t *testing.T) {
	const maxPayload = 1024 - 8
	rgba := make([]byte, FrameSize)
	total := totalChunksFor(FrameSize)

	// All non-final chunks carry exactly maxPayload bytes.
	pkt := buildPacket(rgba, 0, total)
	loLen, hiLen := int(pkt[6]), int(pkt[7])
	gotLen := loLen | (hiLen << 8)
	if gotLen != maxPayload {
		t.Errorf("non-final chunk payload length = %d, want %d", gotLen, maxPayload)
	}

	// Final chunk carries the remainder.
	last := total - 1
	remainder := FrameSize - last*maxPayload
	pktFinal := buildPacket(rgba, last, total)
	loLen, hiLen = int(pktFinal[6]), int(pktFinal[7])
	gotFinal := loLen | (hiLen << 8)
	if gotFinal != remainder {
		t.Errorf("final chunk payload length = %d, want %d", gotFinal, remainder)
	}
}

func TestBuildPacket_RGBAtoBGRA(t *testing.T) {
	// Single pixel: R=0x11 G=0x22 B=0x33 A=0x44
	rgba := make([]byte, FrameSize)
	rgba[0], rgba[1], rgba[2], rgba[3] = 0x11, 0x22, 0x33, 0x44

	pkt := buildPacket(rgba, 0, totalChunksFor(FrameSize))

	const hdr = 8
	if pkt[hdr] != 0x33 {
		t.Errorf("B byte = %02x, want 33", pkt[hdr])
	}
	if pkt[hdr+1] != 0x22 {
		t.Errorf("G byte = %02x, want 22", pkt[hdr+1])
	}
	if pkt[hdr+2] != 0x11 {
		t.Errorf("R byte = %02x, want 11", pkt[hdr+2])
	}
	if pkt[hdr+3] != 0x44 {
		t.Errorf("A byte = %02x, want 44", pkt[hdr+3])
	}
}

func TestBuildPacket_SingleChunkFrame(t *testing.T) {
	// A frame small enough to fit in one chunk.
	const payloadBytes = 8 // 2 RGBA pixels
	rgba := []byte{0x11, 0x22, 0x33, 0x44, 0xAA, 0xBB, 0xCC, 0xDD}
	pkt := buildPacket(rgba, 0, 1)

	if len(pkt) != 1024 {
		t.Fatalf("packet length = %d, want 1024", len(pkt))
	}
	// Final-chunk flag must be set (only chunk == last chunk).
	if pkt[3] != 0x01 {
		t.Errorf("single-chunk final flag = %02x, want 01", pkt[3])
	}
	// Payload length = 8 bytes.
	gotLen := int(pkt[6]) | int(pkt[7])<<8
	if gotLen != payloadBytes {
		t.Errorf("payload length = %d, want %d", gotLen, payloadBytes)
	}
	// BGRA for pixel 0: B=0x33 G=0x22 R=0x11 A=0x44
	const hdr = 8
	if pkt[hdr] != 0x33 || pkt[hdr+1] != 0x22 || pkt[hdr+2] != 0x11 || pkt[hdr+3] != 0x44 {
		t.Errorf("pixel 0 BGRA = %02x %02x %02x %02x, want 33 22 11 44",
			pkt[hdr], pkt[hdr+1], pkt[hdr+2], pkt[hdr+3])
	}
}

func TestBuildPacket_MultiChunk_ChunkCountCorrect(t *testing.T) {
	rgba := make([]byte, FrameSize)
	total := totalChunksFor(FrameSize)
	if total != wantChunks {
		t.Errorf("totalChunksFor(FrameSize) = %d, want %d", total, wantChunks)
	}
	// Verify every chunk index produces a valid 1024-byte packet.
	for i := range total {
		pkt := buildPacket(rgba, i, total)
		if len(pkt) != 1024 {
			t.Errorf("chunk %d: length = %d, want 1024", i, len(pkt))
		}
	}
}

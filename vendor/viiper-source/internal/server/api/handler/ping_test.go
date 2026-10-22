package handler_test

import (
	"encoding/json"
	"testing"

	"github.com/stretchr/testify/assert"

	handlerTest "github.com/DualSenseClient/VIIPER/internal/_testing"
	"github.com/DualSenseClient/VIIPER/internal/server/api"
	"github.com/DualSenseClient/VIIPER/internal/server/api/handler"
	"github.com/DualSenseClient/VIIPER/internal/server/usb"
	"github.com/DualSenseClient/VIIPER/viiperclient"
	"github.com/DualSenseClient/VIIPER/viipertypes"
)

func TestPing(t *testing.T) {
	addr, _, done := handlerTest.StartAPIServer(t, func(r *api.Router, s *usb.Server, apiSrv *api.Server) {
		r.Register("ping", handler.Ping())
	})
	defer done()

	c := viiperclient.NewTransport(addr)
	line, err := c.Do("ping", nil, nil)
	assert.NoError(t, err)

	var out viipertypes.PingResponse
	err = json.Unmarshal([]byte(line), &out)
	assert.NoError(t, err)
	assert.Equal(t, "VIIPER", out.Server)
	assert.NotEmpty(t, out.Version)
}

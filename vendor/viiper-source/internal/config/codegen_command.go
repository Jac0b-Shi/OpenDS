//go:build !release

package config

import "github.com/DualSenseClient/VIIPER/internal/cmd"

type codegenCommand struct {
	Codegen cmd.Codegen `cmd:"" help:"Generate client libraries from server code"`
}

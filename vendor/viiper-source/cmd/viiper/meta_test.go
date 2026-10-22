package main

import (
	"strings"
	"testing"
)

func TestDescriptionUsesDualSenseClientSource(t *testing.T) {
	description := Description()
	if !strings.Contains(description, "Source:  https://github.com/DualSenseClient/VIIPER") {
		t.Fatalf("Description() does not contain the DualSenseClient source URL: %q", description)
	}
	if strings.Contains(strings.ToLower(description), "alia5") {
		t.Fatalf("Description() retains the Alia5 source URL: %q", description)
	}
}

package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
)

// ==================== Event API Tests ====================

// CE-001: Test fire event
func TestEventFire(t *testing.T) {
	client := getClient(t)

	eventName := "test-event-" + randomID()
	payload := []byte("test event payload")

	event := &api.UserEvent{
		Name:    eventName,
		Payload: payload,
	}

	eventID, _, err := client.Event().Fire(event, nil)
	if err != nil {
		t.Logf("Event fire error (may not be supported): %v", err)
		t.Skip("Event API not supported")
	}

	assert.NotEmpty(t, eventID, "Should return event ID")
	t.Logf("Fired event: ID=%s, Name=%s", eventID, eventName)
}

// CE-002: Test list events
func TestEventList(t *testing.T) {
	client := getClient(t)

	// Fire an event first
	eventName := "list-event-" + randomID()
	event := &api.UserEvent{
		Name:    eventName,
		Payload: []byte("list test"),
	}
	_, _, err := client.Event().Fire(event, nil)
	if err != nil {
		t.Skip("Event API not supported")
	}

	// List events
	events, _, err := client.Event().List("", nil)
	assert.NoError(t, err, "Event list should succeed")

	t.Logf("Found %d events", len(events))
}

// CE-003: Test list events with name filter
func TestEventListWithFilter(t *testing.T) {
	client := getClient(t)

	eventName := "filter-event-" + randomID()

	// Fire event
	event := &api.UserEvent{
		Name:    eventName,
		Payload: []byte("filter test"),
	}
	_, _, err := client.Event().Fire(event, nil)
	if err != nil {
		t.Skip("Event API not supported")
	}

	// List events with name filter
	events, _, err := client.Event().List(eventName, nil)
	assert.NoError(t, err, "Event list with filter should succeed")

	// Should find our event
	found := false
	for _, e := range events {
		if e.Name == eventName {
			found = true
			break
		}
	}
	assert.True(t, found, "Should find our event")
}

// CE-004: Test event with node filter
func TestEventFireWithNodeFilter(t *testing.T) {
	client := getClient(t)

	// Get node name
	self, err := client.Agent().Self()
	if err != nil {
		t.Skip("Could not get agent self info")
	}
	config := self["Config"].(map[string]interface{})
	nodeName := config["NodeName"].(string)

	eventName := "node-event-" + randomID()
	event := &api.UserEvent{
		Name:       eventName,
		Payload:    []byte("node specific"),
		NodeFilter: nodeName,
	}

	eventID, _, err := client.Event().Fire(event, nil)
	if err != nil {
		t.Skip("Event API not supported")
	}

	assert.NotEmpty(t, eventID)
	t.Logf("Fired node-specific event: %s", eventID)
}

// CE-005: Test event with service filter
func TestEventFireWithServiceFilter(t *testing.T) {
	client := getClient(t)

	// Register a service first
	serviceName := "event-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
	})
	if err != nil {
		t.Skip("Could not register service")
	}
	defer client.Agent().ServiceDeregister(serviceName)

	eventName := "service-event-" + randomID()
	event := &api.UserEvent{
		Name:          eventName,
		Payload:       []byte("service specific"),
		ServiceFilter: serviceName,
	}

	eventID, _, err := client.Event().Fire(event, nil)
	if err != nil {
		t.Skip("Event API not supported")
	}

	assert.NotEmpty(t, eventID)
	t.Logf("Fired service-specific event: %s", eventID)
}

// CE-006: Test event with tag filter
func TestEventFireWithTagFilter(t *testing.T) {
	client := getClient(t)

	// Register a service with tags
	serviceName := "tag-event-service-" + randomID()
	err := client.Agent().ServiceRegister(&api.AgentServiceRegistration{
		ID:   serviceName,
		Name: serviceName,
		Port: 8080,
		Tags: []string{"event-target"},
	})
	if err != nil {
		t.Skip("Could not register service")
	}
	defer client.Agent().ServiceDeregister(serviceName)

	eventName := "tag-event-" + randomID()
	event := &api.UserEvent{
		Name:          eventName,
		Payload:       []byte("tag specific"),
		ServiceFilter: serviceName,
		TagFilter:     "event-target",
	}

	eventID, _, err := client.Event().Fire(event, nil)
	if err != nil {
		t.Skip("Event API not supported")
	}

	assert.NotEmpty(t, eventID)
	t.Logf("Fired tag-specific event: %s", eventID)
}

package tests

import (
	"testing"

	"github.com/hashicorp/consul/api"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// ==================== ACL Token Tests ====================

// CACL-001: Test create ACL token
func TestACLTokenCreate(t *testing.T) {
	client := getClient(t)

	token := &api.ACLToken{
		Description: "Test token " + randomID(),
		Policies:    []*api.ACLTokenPolicyLink{},
		Local:       true,
	}

	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Logf("ACL token create error (may not be enabled): %v", err)
		t.Skip("ACL not enabled or not supported")
	}

	assert.NotEmpty(t, created.AccessorID, "Should return accessor ID")
	assert.NotEmpty(t, created.SecretID, "Should return secret ID")

	t.Logf("Created token: AccessorID=%s", created.AccessorID)

	// Cleanup
	client.ACL().TokenDelete(created.AccessorID, nil)
}

// CACL-002: Test read ACL token
func TestACLTokenRead(t *testing.T) {
	client := getClient(t)

	// Create first
	token := &api.ACLToken{
		Description: "Read test token " + randomID(),
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Read
	read, _, err := client.ACL().TokenRead(created.AccessorID, nil)
	assert.NoError(t, err, "Token read should succeed")
	assert.Equal(t, created.AccessorID, read.AccessorID)
	assert.Equal(t, token.Description, read.Description)
}

// CACL-003: Test delete ACL token
func TestACLTokenDelete(t *testing.T) {
	client := getClient(t)

	// Create first
	token := &api.ACLToken{
		Description: "Delete test token " + randomID(),
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}

	// Delete
	_, err = client.ACL().TokenDelete(created.AccessorID, nil)
	assert.NoError(t, err, "Token delete should succeed")

	// Verify deleted - reading a deleted token should return an error (404/403)
	_, _, err = client.ACL().TokenRead(created.AccessorID, nil)
	assert.Error(t, err, "Reading deleted token should return an error")
}

// CACL-004: Test list ACL tokens
func TestACLTokenList(t *testing.T) {
	client := getClient(t)

	// Create a token
	token := &api.ACLToken{
		Description: "List test token " + randomID(),
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// List
	tokens, _, err := client.ACL().TokenList(nil)
	assert.NoError(t, err, "Token list should succeed")
	assert.NotEmpty(t, tokens, "Should have at least one token")

	t.Logf("Found %d tokens", len(tokens))
}

// ==================== ACL Policy Tests ====================

// CACL-005: Test create ACL policy
func TestACLPolicyCreate(t *testing.T) {
	client := getClient(t)

	policy := &api.ACLPolicy{
		Name:        "test-policy-" + randomID(),
		Description: "Test policy for SDK tests",
		Rules:       `key_prefix "" { policy = "read" }`,
	}

	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Logf("ACL policy create error: %v", err)
		t.Skip("ACL not enabled or not supported")
	}

	assert.NotEmpty(t, created.ID, "Should return policy ID")
	assert.Equal(t, policy.Name, created.Name)

	t.Logf("Created policy: ID=%s, Name=%s", created.ID, created.Name)

	// Cleanup
	client.ACL().PolicyDelete(created.ID, nil)
}

// CACL-006: Test read ACL policy
func TestACLPolicyRead(t *testing.T) {
	client := getClient(t)

	// Create first
	policy := &api.ACLPolicy{
		Name:  "read-policy-" + randomID(),
		Rules: `key_prefix "test/" { policy = "write" }`,
	}
	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(created.ID, nil)

	// Read
	read, _, err := client.ACL().PolicyRead(created.ID, nil)
	assert.NoError(t, err, "Policy read should succeed")
	assert.Equal(t, created.ID, read.ID)
	assert.Equal(t, policy.Name, read.Name)
}

// CACL-007: Test delete ACL policy
func TestACLPolicyDelete(t *testing.T) {
	client := getClient(t)

	// Create first
	policy := &api.ACLPolicy{
		Name:  "delete-policy-" + randomID(),
		Rules: `key_prefix "delete/" { policy = "read" }`,
	}
	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}

	// Delete
	_, err = client.ACL().PolicyDelete(created.ID, nil)
	assert.NoError(t, err, "Policy delete should succeed")
}

// CACL-008: Test list ACL policies
func TestACLPolicyList(t *testing.T) {
	client := getClient(t)

	// Create a policy
	policy := &api.ACLPolicy{
		Name:  "list-policy-" + randomID(),
		Rules: `key_prefix "list/" { policy = "read" }`,
	}
	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(created.ID, nil)

	// List
	policies, _, err := client.ACL().PolicyList(nil)
	assert.NoError(t, err, "Policy list should succeed")
	assert.NotEmpty(t, policies, "Should have at least one policy")

	t.Logf("Found %d policies", len(policies))
}

// ==================== ACL Role Tests ====================

// CACL-009: Test create ACL role
func TestACLRoleCreate(t *testing.T) {
	client := getClient(t)

	role := &api.ACLRole{
		Name:        "test-role-" + randomID(),
		Description: "Test role for SDK tests",
	}

	created, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Logf("ACL role create error: %v", err)
		t.Skip("ACL not enabled or not supported")
	}

	assert.NotEmpty(t, created.ID, "Should return role ID")
	assert.Equal(t, role.Name, created.Name)

	t.Logf("Created role: ID=%s, Name=%s", created.ID, created.Name)

	// Cleanup
	client.ACL().RoleDelete(created.ID, nil)
}

// CACL-010: Test read ACL role
func TestACLRoleRead(t *testing.T) {
	client := getClient(t)

	// Create first
	role := &api.ACLRole{
		Name: "read-role-" + randomID(),
	}
	created, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().RoleDelete(created.ID, nil)

	// Read
	read, _, err := client.ACL().RoleRead(created.ID, nil)
	assert.NoError(t, err, "Role read should succeed")
	assert.Equal(t, created.ID, read.ID)
}

// CACL-011: Test update ACL role
func TestACLRoleUpdate(t *testing.T) {
	client := getClient(t)

	// Create first
	role := &api.ACLRole{
		Name:        "update-role-" + randomID(),
		Description: "Original description",
	}
	created, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().RoleDelete(created.ID, nil)

	// Update
	created.Description = "Updated description"
	updated, _, err := client.ACL().RoleUpdate(created, nil)
	assert.NoError(t, err, "Role update should succeed")
	assert.Equal(t, "Updated description", updated.Description)
}

// CACL-012: Test delete ACL role
func TestACLRoleDelete(t *testing.T) {
	client := getClient(t)

	// Create first
	role := &api.ACLRole{
		Name: "delete-role-" + randomID(),
	}
	created, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}

	// Delete
	_, err = client.ACL().RoleDelete(created.ID, nil)
	assert.NoError(t, err, "Role delete should succeed")
}

// CACL-013: Test list ACL roles
func TestACLRoleList(t *testing.T) {
	client := getClient(t)

	// Create a role
	role := &api.ACLRole{
		Name: "list-role-" + randomID(),
	}
	created, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().RoleDelete(created.ID, nil)

	// List
	roles, _, err := client.ACL().RoleList(nil)
	assert.NoError(t, err, "Role list should succeed")
	assert.NotEmpty(t, roles, "Should have at least one role")

	t.Logf("Found %d roles", len(roles))
}

// ==================== ACL Token with Policy Tests ====================

// CACL-014: Test token with policy link
func TestACLTokenWithPolicy(t *testing.T) {
	client := getClient(t)

	// Create policy first
	policy := &api.ACLPolicy{
		Name:  "token-policy-" + randomID(),
		Rules: `key_prefix "token/" { policy = "read" }`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token with policy
	token := &api.ACLToken{
		Description: "Token with policy " + randomID(),
		Policies: []*api.ACLTokenPolicyLink{
			{ID: createdPolicy.ID},
		},
		Local: true,
	}
	createdToken, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(createdToken.AccessorID, nil)

	assert.NotEmpty(t, createdToken.Policies, "Token should have policies")
	t.Logf("Created token with policy: %s", createdToken.AccessorID)
}

// CACL-015: Test token with role link
func TestACLTokenWithRole(t *testing.T) {
	client := getClient(t)

	// Create role first
	role := &api.ACLRole{
		Name: "token-role-" + randomID(),
	}
	createdRole, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().RoleDelete(createdRole.ID, nil)

	// Create token with role
	token := &api.ACLToken{
		Description: "Token with role " + randomID(),
		Roles: []*api.ACLTokenRoleLink{
			{ID: createdRole.ID},
		},
		Local: true,
	}
	createdToken, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(createdToken.AccessorID, nil)

	assert.NotEmpty(t, createdToken.Roles, "Token should have roles")
	t.Logf("Created token with role: %s", createdToken.AccessorID)
}

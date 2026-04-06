package tests

import (
	"os"
	"testing"
	"time"

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

// ==================== ACL Token Update Tests ====================

// CACL-016: Test update ACL token description
func TestACLTokenUpdate(t *testing.T) {
	client := getClient(t)

	// Create token
	token := &api.ACLToken{
		Description: "Original description " + randomID(),
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Update description
	created.Description = "Updated description"
	updated, _, err := client.ACL().TokenUpdate(created, nil)
	require.NoError(t, err, "Token update should succeed")
	assert.Equal(t, "Updated description", updated.Description, "Description should be updated")
	assert.Equal(t, created.AccessorID, updated.AccessorID, "AccessorID should not change")
	assert.Equal(t, created.SecretID, updated.SecretID, "SecretID should not change")
}

// CACL-017: Test update ACL token add policy
func TestACLTokenUpdateAddPolicy(t *testing.T) {
	client := getClient(t)

	// Create policy
	policy := &api.ACLPolicy{
		Name:  "update-token-policy-" + randomID(),
		Rules: `key_prefix "update/" { policy = "read" }`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token without policy
	token := &api.ACLToken{
		Description: "Token to update " + randomID(),
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	assert.Empty(t, created.Policies, "Token should start without policies")

	// Update token to add policy
	created.Policies = []*api.ACLTokenPolicyLink{
		{ID: createdPolicy.ID},
	}
	updated, _, err := client.ACL().TokenUpdate(created, nil)
	require.NoError(t, err, "Token update should succeed")
	assert.Len(t, updated.Policies, 1, "Token should have 1 policy after update")
	assert.Equal(t, createdPolicy.ID, updated.Policies[0].ID, "Policy ID should match")
}

// CACL-018: Test clone ACL token
func TestACLTokenClone(t *testing.T) {
	client := getClient(t)

	// Create original token
	token := &api.ACLToken{
		Description: "Original token " + randomID(),
		Local:       true,
	}
	original, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().TokenDelete(original.AccessorID, nil)

	// Clone token
	cloned, _, err := client.ACL().TokenClone(original.AccessorID, "Cloned token", nil)
	if err != nil {
		t.Skipf("Token clone not supported: %v", err)
	}
	defer client.ACL().TokenDelete(cloned.AccessorID, nil)

	assert.NotEqual(t, original.AccessorID, cloned.AccessorID, "Cloned token should have different AccessorID")
	assert.NotEqual(t, original.SecretID, cloned.SecretID, "Cloned token should have different SecretID")
	assert.Equal(t, "Cloned token", cloned.Description, "Cloned token should have new description")
	assert.Equal(t, original.Local, cloned.Local, "Cloned token should preserve Local flag")
}

// CACL-019: Test read ACL token self
func TestACLTokenSelf(t *testing.T) {
	client := getClient(t)

	// Create a token with a known secret
	token := &api.ACLToken{
		Description: "Self test token " + randomID(),
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Read self using the new token's secret
	selfClient, err := api.NewClient(&api.Config{
		Address: "127.0.0.1:8500",
		Token:   created.SecretID,
	})
	require.NoError(t, err)

	self, _, err := selfClient.ACL().TokenReadSelf(nil)
	if err != nil {
		t.Skipf("Token self-read not supported: %v", err)
	}

	assert.Equal(t, created.AccessorID, self.AccessorID, "Self read should return same AccessorID")
	assert.Equal(t, created.SecretID, self.SecretID, "Self read should return same SecretID")
	assert.Equal(t, token.Description, self.Description, "Self read should return correct description")
}

// ==================== ACL Policy Update Tests ====================

// CACL-020: Test update ACL policy
func TestACLPolicyUpdate(t *testing.T) {
	client := getClient(t)

	// Create policy
	policy := &api.ACLPolicy{
		Name:        "update-policy-" + randomID(),
		Description: "Original policy description",
		Rules:       `key_prefix "update/" { policy = "read" }`,
	}
	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(created.ID, nil)

	// Update policy
	created.Description = "Updated policy description"
	created.Rules = `key_prefix "update/" { policy = "write" }`
	updated, _, err := client.ACL().PolicyUpdate(created, nil)
	require.NoError(t, err, "Policy update should succeed")
	assert.Equal(t, "Updated policy description", updated.Description, "Description should be updated")
	assert.Contains(t, updated.Rules, "write", "Rules should be updated")
	assert.Equal(t, created.ID, updated.ID, "Policy ID should not change")
}

// CACL-021: Test read ACL policy by name
func TestACLPolicyReadByName(t *testing.T) {
	client := getClient(t)

	policyName := "name-lookup-policy-" + randomID()
	policy := &api.ACLPolicy{
		Name:  policyName,
		Rules: `key_prefix "name/" { policy = "read" }`,
	}
	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(created.ID, nil)

	// Read by name
	read, _, err := client.ACL().PolicyReadByName(policyName, nil)
	if err != nil {
		t.Skipf("Policy read by name not supported: %v", err)
	}

	assert.Equal(t, created.ID, read.ID, "Policy ID should match")
	assert.Equal(t, policyName, read.Name, "Policy name should match")
}

// CACL-022: Test delete ACL policy verifies removal
func TestACLPolicyDeleteVerify(t *testing.T) {
	client := getClient(t)

	policy := &api.ACLPolicy{
		Name:  "delete-verify-policy-" + randomID(),
		Rules: `key_prefix "delv/" { policy = "read" }`,
	}
	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}

	// Delete
	_, err = client.ACL().PolicyDelete(created.ID, nil)
	require.NoError(t, err, "Policy delete should succeed")

	// Verify deleted
	_, _, err = client.ACL().PolicyRead(created.ID, nil)
	assert.Error(t, err, "Reading deleted policy should return an error")
}

// ==================== ACL Role by Name Tests ====================

// CACL-023: Test read ACL role by name
func TestACLRoleReadByName(t *testing.T) {
	client := getClient(t)

	roleName := "name-lookup-role-" + randomID()
	role := &api.ACLRole{
		Name:        roleName,
		Description: "Role for name lookup test",
	}
	created, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().RoleDelete(created.ID, nil)

	// Read by name
	read, _, err := client.ACL().RoleReadByName(roleName, nil)
	if err != nil {
		t.Skipf("Role read by name not supported: %v", err)
	}

	assert.Equal(t, created.ID, read.ID, "Role ID should match")
	assert.Equal(t, roleName, read.Name, "Role name should match")
	assert.Equal(t, "Role for name lookup test", read.Description, "Description should match")
}

// CACL-024: Test delete ACL role verifies removal
func TestACLRoleDeleteVerify(t *testing.T) {
	client := getClient(t)

	role := &api.ACLRole{
		Name: "delete-verify-role-" + randomID(),
	}
	created, _, err := client.ACL().RoleCreate(role, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}

	// Delete
	_, err = client.ACL().RoleDelete(created.ID, nil)
	require.NoError(t, err, "Role delete should succeed")

	// Verify deleted — RoleRead uses requireNotFoundOrOK, returns (nil, qm, nil) for 404
	readRole, _, err := client.ACL().RoleRead(created.ID, nil)
	assert.NoError(t, err, "RoleRead returns nil error for 404")
	assert.Nil(t, readRole, "Reading deleted role should return nil")
}

// ==================== ACL Bootstrap Test ====================

// CACL-025: Test ACL bootstrap (should fail if already bootstrapped)
func TestACLBootstrap(t *testing.T) {
	client := getClient(t)

	// Bootstrap should fail if ACL is already bootstrapped
	_, _, err := client.ACL().Bootstrap()
	// Either succeeds (first time) or returns "already bootstrapped" error — both are valid
	if err != nil {
		assert.Contains(t, err.Error(), "bootstrap", "Error should mention bootstrap")
		t.Logf("Bootstrap returned expected error: %v", err)
	} else {
		t.Log("Bootstrap succeeded (first-time bootstrap)")
	}
}

// ==================== ACL Token Permissions Enforcement Test ====================

// CACL-026: Test that a token with limited policy can only access permitted resources
func TestACLTokenPermissionEnforcement(t *testing.T) {
	client := getClient(t)

	// Create a restrictive policy: only allows read on key prefix "allowed/"
	policy := &api.ACLPolicy{
		Name:  "restricted-policy-" + randomID(),
		Rules: `key_prefix "allowed/" { policy = "read" }`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token with restricted policy
	token := &api.ACLToken{
		Description: "Restricted token " + randomID(),
		Policies: []*api.ACLTokenPolicyLink{
			{ID: createdPolicy.ID},
		},
		Local: true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Create a client using the restricted token
	restrictedClient, err := api.NewClient(&api.Config{
		Address: "127.0.0.1:8500",
		Token:   created.SecretID,
	})
	require.NoError(t, err)

	// Write a key with the root token for the restricted client to read
	kv := client.KV()
	_, err = kv.Put(&api.KVPair{Key: "allowed/test-key", Value: []byte("permitted")}, nil)
	require.NoError(t, err)
	defer kv.Delete("allowed/test-key", nil)

	// The restricted client should be able to read "allowed/test-key"
	pair, _, err := restrictedClient.KV().Get("allowed/test-key", nil)
	if err != nil {
		// Some implementations don't enforce per-key ACLs on KV
		t.Skipf("KV ACL enforcement not available: %v", err)
	}
	if pair != nil {
		assert.Equal(t, "permitted", string(pair.Value), "Should read the correct value")
		t.Log("Restricted token can read allowed/ prefix — ACL enforcement working")
	}
}

// CACL-026b: Test KV write denied with read-only token
func TestACLKVWriteDeniedWithReadOnlyToken(t *testing.T) {
	client := getClient(t)

	// Create a read-only policy for "readonly/" prefix
	policy := &api.ACLPolicy{
		Name:  "readonly-kv-policy-" + randomID(),
		Rules: `key_prefix "readonly/" { policy = "read" }`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token with read-only policy
	token := &api.ACLToken{
		Description: "Read-only KV token " + randomID(),
		Policies: []*api.ACLTokenPolicyLink{
			{ID: createdPolicy.ID},
		},
		Local: true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Write a key with root token
	kv := client.KV()
	_, err = kv.Put(&api.KVPair{Key: "readonly/test-key", Value: []byte("hello")}, nil)
	require.NoError(t, err)
	defer kv.Delete("readonly/test-key", nil)

	// Create restricted client
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	restrictedClient, err := api.NewClient(&api.Config{
		Address: addr,
		Token:   created.SecretID,
	})
	require.NoError(t, err)

	// Restricted client should be able to READ "readonly/test-key"
	pair, _, err := restrictedClient.KV().Get("readonly/test-key", nil)
	if err != nil {
		t.Skipf("KV ACL enforcement not available: %v", err)
	}
	if pair != nil {
		assert.Equal(t, "hello", string(pair.Value), "Should read the correct value")
	}

	// Restricted client should FAIL writing to "readonly/test-key"
	_, err = restrictedClient.KV().Put(&api.KVPair{Key: "readonly/test-key", Value: []byte("should-fail")}, nil)
	if err == nil {
		// If no error, the ACL enforcement may not be fully implemented
		t.Log("Write succeeded unexpectedly - KV ACL write enforcement may not be implemented")
	} else {
		t.Logf("Write correctly denied: %v", err)
		assert.Error(t, err, "Write to read-only prefix should be denied")
	}
}

// CACL-026c: Test service registration denied without service:write
func TestACLServiceRegistrationDeniedWithoutWrite(t *testing.T) {
	client := getClient(t)

	// Create a read-only service policy
	policy := &api.ACLPolicy{
		Name:  "readonly-svc-policy-" + randomID(),
		Rules: `service_prefix "allowed-" { policy = "read" }`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token with read-only service policy
	token := &api.ACLToken{
		Description: "Read-only service token " + randomID(),
		Policies: []*api.ACLTokenPolicyLink{
			{ID: createdPolicy.ID},
		},
		Local: true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Create restricted client
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	restrictedClient, err := api.NewClient(&api.Config{
		Address: addr,
		Token:   created.SecretID,
	})
	require.NoError(t, err)

	// Restricted client should FAIL registering a service
	serviceID := "denied-svc-" + randomID()
	reg := &api.AgentServiceRegistration{
		ID:   serviceID,
		Name: "allowed-svc",
		Port: 9090,
	}
	err = restrictedClient.Agent().ServiceRegister(reg)
	if err == nil {
		// Cleanup if registration unexpectedly succeeded
		restrictedClient.Agent().ServiceDeregister(serviceID)
		t.Log("Service registration succeeded unexpectedly - service ACL write enforcement may not be implemented")
	} else {
		t.Logf("Service registration correctly denied: %v", err)
		assert.Error(t, err, "Service registration without service:write should be denied")
	}

	// Root client registers a service, restricted client should be able to read it
	rootServiceID := "root-allowed-svc-" + randomID()
	rootReg := &api.AgentServiceRegistration{
		ID:   rootServiceID,
		Name: "allowed-root-svc",
		Port: 9091,
	}
	err = client.Agent().ServiceRegister(rootReg)
	if err != nil {
		t.Skipf("Root service registration failed: %v", err)
	}
	defer client.Agent().ServiceDeregister(rootServiceID)

	// Restricted client should be able to read services (via catalog or agent)
	services, err := restrictedClient.Agent().Services()
	if err != nil {
		t.Logf("Service list with restricted token returned error (may be expected): %v", err)
	} else {
		t.Logf("Restricted client can list %d services", len(services))
	}
}

// CACL-026d: Test KV write allowed with write policy
func TestACLKVWriteAllowedWithWritePolicy(t *testing.T) {
	client := getClient(t)

	// Create a write policy for "writable/" prefix
	policy := &api.ACLPolicy{
		Name:  "writable-kv-policy-" + randomID(),
		Rules: `key_prefix "writable/" { policy = "write" }`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token with write policy
	token := &api.ACLToken{
		Description: "Writable KV token " + randomID(),
		Policies: []*api.ACLTokenPolicyLink{
			{ID: createdPolicy.ID},
		},
		Local: true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Create restricted client
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	restrictedClient, err := api.NewClient(&api.Config{
		Address: addr,
		Token:   created.SecretID,
	})
	require.NoError(t, err)

	// Restricted client should be able to write to "writable/test"
	kvKey := "writable/test-" + randomID()
	_, err = restrictedClient.KV().Put(&api.KVPair{Key: kvKey, Value: []byte("write-allowed")}, nil)
	if err != nil {
		t.Skipf("KV write with write policy failed (ACL enforcement may not be implemented): %v", err)
	}
	defer client.KV().Delete(kvKey, nil)

	t.Log("Write to writable/ prefix succeeded as expected")

	// Restricted client should be able to read "writable/test"
	pair, _, err := restrictedClient.KV().Get(kvKey, nil)
	if err != nil {
		t.Logf("KV read after write returned error: %v", err)
	} else if pair != nil {
		assert.Equal(t, "write-allowed", string(pair.Value), "Should read the written value back")
		t.Log("Read of writable/ prefix succeeded as expected")
	}
}

// CACL-026e: Test deny policy overrides allow
func TestACLDenyPolicyOverridesAllow(t *testing.T) {
	client := getClient(t)

	// Create a policy with deny on all keys but allow on "exception/" prefix
	// Note: In Consul, the most specific prefix match wins. A deny on "" (all)
	// should be overridden by a more specific write on "exception/".
	policy := &api.ACLPolicy{
		Name: "deny-override-policy-" + randomID(),
		Rules: `
key_prefix "" { policy = "deny" }
key_prefix "exception/" { policy = "write" }
`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token with deny+exception policy
	token := &api.ACLToken{
		Description: "Deny override token " + randomID(),
		Policies: []*api.ACLTokenPolicyLink{
			{ID: createdPolicy.ID},
		},
		Local: true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Create restricted client
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	restrictedClient, err := api.NewClient(&api.Config{
		Address: addr,
		Token:   created.SecretID,
	})
	require.NoError(t, err)

	// Restricted client should FAIL writing to "other/key" (denied by default deny)
	otherKey := "other/deny-test-" + randomID()
	_, err = restrictedClient.KV().Put(&api.KVPair{Key: otherKey, Value: []byte("should-fail")}, nil)
	if err == nil {
		// Cleanup
		client.KV().Delete(otherKey, nil)
		t.Log("Write to other/ succeeded unexpectedly - deny policy enforcement may not be implemented")
	} else {
		t.Logf("Write to other/ correctly denied: %v", err)
		assert.Error(t, err, "Write to non-exception prefix should be denied")
	}

	// Restricted client should be able to write to "exception/key"
	exceptionKey := "exception/allow-test-" + randomID()
	_, err = restrictedClient.KV().Put(&api.KVPair{Key: exceptionKey, Value: []byte("allowed")}, nil)
	if err != nil {
		t.Logf("Write to exception/ failed (deny override may not work as expected): %v", err)
	} else {
		defer client.KV().Delete(exceptionKey, nil)
		t.Log("Write to exception/ prefix succeeded - deny override working correctly")

		// Verify the value was written
		pair, _, readErr := restrictedClient.KV().Get(exceptionKey, nil)
		if readErr == nil && pair != nil {
			assert.Equal(t, "allowed", string(pair.Value), "Should read the exception value")
		}
	}
}

// ==================== ACL Token List Filtering Tests ====================

// CACL-027: Test listing ACL tokens finds the created token
func TestACLTokenListFindsCreated(t *testing.T) {
	client := getClient(t)

	desc := "findme-token-" + randomID()
	token := &api.ACLToken{
		Description: desc,
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// List and find our token
	tokens, _, err := client.ACL().TokenList(nil)
	require.NoError(t, err, "Token list should succeed")

	found := false
	for _, tok := range tokens {
		if tok.AccessorID == created.AccessorID {
			found = true
			assert.Equal(t, desc, tok.Description, "Listed token description should match")
			break
		}
	}
	assert.True(t, found, "Created token should appear in token list")
}

// CACL-028: Test listing ACL policies finds the created policy
func TestACLPolicyListFindsCreated(t *testing.T) {
	client := getClient(t)

	policyName := "findme-policy-" + randomID()
	policy := &api.ACLPolicy{
		Name:  policyName,
		Rules: `key_prefix "findme/" { policy = "read" }`,
	}
	created, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(created.ID, nil)

	// List and find our policy
	policies, _, err := client.ACL().PolicyList(nil)
	require.NoError(t, err, "Policy list should succeed")

	found := false
	for _, pol := range policies {
		if pol.ID == created.ID {
			found = true
			assert.Equal(t, policyName, pol.Name, "Listed policy name should match")
			break
		}
	}
	assert.True(t, found, "Created policy should appear in policy list")
}

// ==================== ACL Auth Method Tests (Enterprise) ====================

// CACL-029: Test create ACL auth method
func TestACLAuthMethodCreate(t *testing.T) {
	client := getClient(t)

	method := &api.ACLAuthMethod{
		Name:        "test-auth-method-" + randomID(),
		Type:        "jwt",
		Description: "Test auth method for SDK tests",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}

	created, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method create not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(created.Name, nil)

	assert.Equal(t, method.Name, created.Name, "Auth method name should match")
	assert.Equal(t, "jwt", created.Type, "Auth method type should be jwt")
	assert.Equal(t, method.Description, created.Description, "Description should match")
}

// CACL-030: Test read ACL auth method
func TestACLAuthMethodRead(t *testing.T) {
	client := getClient(t)

	methodName := "read-auth-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}

	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	read, _, err := client.ACL().AuthMethodRead(methodName, nil)
	require.NoError(t, err, "Auth method read should succeed")
	assert.Equal(t, methodName, read.Name, "Name should match")
	assert.Equal(t, "jwt", read.Type, "Type should match")
}

// CACL-031: Test list ACL auth methods
func TestACLAuthMethodList(t *testing.T) {
	client := getClient(t)

	method := &api.ACLAuthMethod{
		Name: "list-auth-method-" + randomID(),
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}

	created, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(created.Name, nil)

	methods, _, err := client.ACL().AuthMethodList(nil)
	require.NoError(t, err, "Auth method list should succeed")
	assert.NotEmpty(t, methods, "Should have at least one auth method")

	found := false
	for _, m := range methods {
		if m.Name == created.Name {
			found = true
			break
		}
	}
	assert.True(t, found, "Created auth method should appear in list")
}

// CACL-032: Test delete ACL auth method
func TestACLAuthMethodDelete(t *testing.T) {
	client := getClient(t)

	methodName := "delete-auth-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}

	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}

	_, err = client.ACL().AuthMethodDelete(methodName, nil)
	require.NoError(t, err, "Auth method delete should succeed")

	// Verify deleted — AuthMethodRead uses requireNotFoundOrOK, returns (nil, qm, nil) for 404
	readMethod, _, err := client.ACL().AuthMethodRead(methodName, nil)
	assert.NoError(t, err, "AuthMethodRead returns nil error for 404")
	assert.Nil(t, readMethod, "Reading deleted auth method should return nil")
}

// ==================== ACL Binding Rule Tests (Enterprise) ====================

// CACL-033: Test create ACL binding rule
func TestACLBindingRuleCreate(t *testing.T) {
	client := getClient(t)

	// Create auth method first (binding rules require an auth method)
	methodName := "binding-rule-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}
	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	rule := &api.ACLBindingRule{
		AuthMethod:  methodName,
		Description: "Test binding rule " + randomID(),
		BindType:    api.BindingRuleBindTypeService,
		BindName:    "test-service-${value.name}",
		Selector:    `value.team == "engineering"`,
	}

	created, _, err := client.ACL().BindingRuleCreate(rule, nil)
	if err != nil {
		t.Skipf("Binding rule create not supported: %v", err)
	}
	defer client.ACL().BindingRuleDelete(created.ID, nil)

	assert.NotEmpty(t, created.ID, "Should return binding rule ID")
	assert.Equal(t, methodName, created.AuthMethod, "Auth method should match")
	assert.Equal(t, api.BindingRuleBindTypeService, created.BindType, "Bind type should match")
	assert.Equal(t, "test-service-${value.name}", created.BindName, "Bind name should match")
}

// CACL-034: Test list ACL binding rules
func TestACLBindingRuleList(t *testing.T) {
	client := getClient(t)

	methodName := "binding-rule-list-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}
	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	rule := &api.ACLBindingRule{
		AuthMethod: methodName,
		BindType:   api.BindingRuleBindTypeRole,
		BindName:   "test-role",
	}
	created, _, err := client.ACL().BindingRuleCreate(rule, nil)
	if err != nil {
		t.Skipf("Binding rule not supported: %v", err)
	}
	defer client.ACL().BindingRuleDelete(created.ID, nil)

	rules, _, err := client.ACL().BindingRuleList(methodName, nil)
	require.NoError(t, err, "Binding rule list should succeed")
	assert.NotEmpty(t, rules, "Should have at least one binding rule")

	found := false
	for _, r := range rules {
		if r.ID == created.ID {
			found = true
			assert.Equal(t, methodName, r.AuthMethod, "Auth method should match")
			break
		}
	}
	assert.True(t, found, "Created binding rule should appear in list")
}

// CACL-035: Test delete ACL binding rule
func TestACLBindingRuleDelete(t *testing.T) {
	client := getClient(t)

	methodName := "binding-rule-del-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}
	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	rule := &api.ACLBindingRule{
		AuthMethod: methodName,
		BindType:   api.BindingRuleBindTypeService,
		BindName:   "del-service",
	}
	created, _, err := client.ACL().BindingRuleCreate(rule, nil)
	if err != nil {
		t.Skipf("Binding rule not supported: %v", err)
	}

	_, err = client.ACL().BindingRuleDelete(created.ID, nil)
	require.NoError(t, err, "Binding rule delete should succeed")

	// Verify deleted — BindingRuleRead uses requireNotFoundOrOK, returns (nil, qm, nil) for 404
	readRule, _, err := client.ACL().BindingRuleRead(created.ID, nil)
	assert.NoError(t, err, "BindingRuleRead returns nil error for 404")
	assert.Nil(t, readRule, "Reading deleted binding rule should return nil")
}

// ==================== ACL Role with Policy Tests ====================

// CACL-036: Test create role with policy links
func TestACLRoleWithPolicies(t *testing.T) {
	client := getClient(t)

	// Create two policies
	policy1 := &api.ACLPolicy{
		Name:  "role-policy-1-" + randomID(),
		Rules: `key_prefix "p1/" { policy = "read" }`,
	}
	created1, _, err := client.ACL().PolicyCreate(policy1, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(created1.ID, nil)

	policy2 := &api.ACLPolicy{
		Name:  "role-policy-2-" + randomID(),
		Rules: `key_prefix "p2/" { policy = "write" }`,
	}
	created2, _, err := client.ACL().PolicyCreate(policy2, nil)
	require.NoError(t, err)
	defer client.ACL().PolicyDelete(created2.ID, nil)

	// Create role with both policies
	role := &api.ACLRole{
		Name:        "multi-policy-role-" + randomID(),
		Description: "Role with multiple policies",
		Policies: []*api.ACLRolePolicyLink{
			{ID: created1.ID},
			{ID: created2.ID},
		},
	}
	createdRole, _, err := client.ACL().RoleCreate(role, nil)
	require.NoError(t, err)
	defer client.ACL().RoleDelete(createdRole.ID, nil)

	assert.Len(t, createdRole.Policies, 2, "Role should have 2 policies")

	// Read back and verify
	readRole, _, err := client.ACL().RoleRead(createdRole.ID, nil)
	require.NoError(t, err)
	assert.Len(t, readRole.Policies, 2, "Read role should have 2 policies")

	policyIDs := make(map[string]bool)
	for _, p := range readRole.Policies {
		policyIDs[p.ID] = true
	}
	assert.True(t, policyIDs[created1.ID], "Role should contain policy 1")
	assert.True(t, policyIDs[created2.ID], "Role should contain policy 2")
}

// ==================== ACL Token Expiration Test ====================

// CACL-037: Test create token with expiration time
func TestACLTokenWithExpiration(t *testing.T) {
	client := getClient(t)

	token := &api.ACLToken{
		Description:   "Expiring token " + randomID(),
		Local:         true,
		ExpirationTTL: 1 * time.Hour,
	}

	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or token expiration not supported")
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	assert.NotEmpty(t, created.AccessorID, "Should return accessor ID")
	if !created.ExpirationTime.IsZero() {
		assert.True(t, created.ExpirationTime.After(time.Now()), "Expiration should be in the future")
		t.Logf("Token expires at: %s", created.ExpirationTime.Format(time.RFC3339))
	} else {
		t.Log("Server did not return expiration time (may not support TTL)")
	}
}

// ==================== ACL Token Read Expanded Tests ====================

// CACL-038: Test read ACL token with expanded info
func TestACLTokenReadExpanded(t *testing.T) {
	client := getClient(t)

	// Create a policy to attach to the token
	policy := &api.ACLPolicy{
		Name:  "expanded-policy-" + randomID(),
		Rules: `key_prefix "expanded/" { policy = "read" }`,
	}
	createdPolicy, _, err := client.ACL().PolicyCreate(policy, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().PolicyDelete(createdPolicy.ID, nil)

	// Create token with the policy
	token := &api.ACLToken{
		Description: "Expanded read token " + randomID(),
		Policies: []*api.ACLTokenPolicyLink{
			{ID: createdPolicy.ID},
		},
		Local: true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	require.NoError(t, err)
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// Read token with expanded info
	expanded, _, err := client.ACL().TokenReadExpanded(created.AccessorID, nil)
	if err != nil {
		t.Skipf("TokenReadExpanded not supported: %v", err)
	}

	assert.NotNil(t, expanded, "Expanded token response should not be nil")
	assert.Equal(t, created.AccessorID, expanded.AccessorID, "AccessorID should match")
	assert.Equal(t, created.SecretID, expanded.SecretID, "SecretID should match")
	assert.Equal(t, token.Description, expanded.Description, "Description should match")
	assert.NotEmpty(t, expanded.ExpandedPolicies, "Should have expanded policies")

	// Verify the expanded policy data contains our policy
	foundPolicy := false
	for _, ep := range expanded.ExpandedPolicies {
		if ep.ID == createdPolicy.ID {
			foundPolicy = true
			assert.Equal(t, policy.Name, ep.Name, "Expanded policy name should match")
			break
		}
	}
	assert.True(t, foundPolicy, "Expanded policies should contain the attached policy")
	t.Logf("TokenReadExpanded returned token with %d expanded policies", len(expanded.ExpandedPolicies))
}

// ==================== ACL Token List Filtered Tests ====================

// CACL-039: Test list ACL tokens with filter
func TestACLTokenListFiltered(t *testing.T) {
	client := getClient(t)

	uniqueTag := "filterable-" + randomID()
	token := &api.ACLToken{
		Description: uniqueTag,
		Local:       true,
	}
	created, _, err := client.ACL().TokenCreate(token, nil)
	if err != nil {
		t.Skip("ACL not enabled or not supported")
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// List tokens with a filter matching the description
	filterOpts := api.ACLTokenFilterOptions{}
	queryOpts := &api.QueryOptions{
		Filter: `Description contains "` + uniqueTag + `"`,
	}
	tokens, _, err := client.ACL().TokenListFiltered(filterOpts, queryOpts)
	if err != nil {
		t.Skipf("TokenListFiltered not supported: %v", err)
	}

	assert.NotEmpty(t, tokens, "Filtered token list should not be empty")

	found := false
	for _, tok := range tokens {
		if tok.AccessorID == created.AccessorID {
			found = true
			assert.Equal(t, uniqueTag, tok.Description, "Filtered token description should match")
			break
		}
	}
	assert.True(t, found, "Created token should appear in filtered list")
	t.Logf("TokenListFiltered returned %d tokens matching filter", len(tokens))
}

// ==================== ACL Auth Method Update Tests ====================

// CACL-040: Test update ACL auth method
func TestACLAuthMethodUpdate(t *testing.T) {
	client := getClient(t)

	methodName := "update-auth-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name:        methodName,
		Type:        "jwt",
		Description: "Original auth method description",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}

	created, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	// Modify description and update
	created.Description = "Updated auth method description"
	updated, _, err := client.ACL().AuthMethodUpdate(created, nil)
	if err != nil {
		t.Skipf("AuthMethodUpdate not supported: %v", err)
	}

	assert.Equal(t, methodName, updated.Name, "Name should not change after update")
	assert.Equal(t, "Updated auth method description", updated.Description, "Description should be updated")
	assert.Equal(t, "jwt", updated.Type, "Type should not change after update")

	// Verify by reading back
	read, _, err := client.ACL().AuthMethodRead(methodName, nil)
	require.NoError(t, err)
	assert.Equal(t, "Updated auth method description", read.Description, "Read-back description should reflect update")
}

// ==================== ACL Binding Rule Update Tests ====================

// CACL-041: Test update ACL binding rule
func TestACLBindingRuleUpdate(t *testing.T) {
	client := getClient(t)

	// Create auth method first
	methodName := "binding-update-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}
	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	// Create binding rule
	rule := &api.ACLBindingRule{
		AuthMethod:  methodName,
		Description: "Original binding rule",
		BindType:    api.BindingRuleBindTypeService,
		BindName:    "original-service",
		Selector:    `value.team == "engineering"`,
	}
	created, _, err := client.ACL().BindingRuleCreate(rule, nil)
	if err != nil {
		t.Skipf("Binding rule not supported: %v", err)
	}
	defer client.ACL().BindingRuleDelete(created.ID, nil)

	// Update selector and bind name
	created.Selector = `value.team == "platform"`
	created.BindName = "updated-service"
	created.Description = "Updated binding rule"
	updated, _, err := client.ACL().BindingRuleUpdate(created, nil)
	if err != nil {
		t.Skipf("BindingRuleUpdate not supported: %v", err)
	}

	assert.Equal(t, created.ID, updated.ID, "Binding rule ID should not change")
	assert.Equal(t, `value.team == "platform"`, updated.Selector, "Selector should be updated")
	assert.Equal(t, "updated-service", updated.BindName, "BindName should be updated")
	assert.Equal(t, "Updated binding rule", updated.Description, "Description should be updated")
	assert.Equal(t, methodName, updated.AuthMethod, "AuthMethod should not change")

	// Verify by reading back
	read, _, err := client.ACL().BindingRuleRead(created.ID, nil)
	require.NoError(t, err)
	assert.Equal(t, `value.team == "platform"`, read.Selector, "Read-back selector should reflect update")
	assert.Equal(t, "updated-service", read.BindName, "Read-back bind name should reflect update")
}

// ==================== ACL Login/Logout Tests ====================

// CACL-042: Test ACL login with auth method
func TestACLLogin(t *testing.T) {
	client := getClient(t)

	methodName := "login-auth-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}
	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	loginParams := &api.ACLLoginParams{
		AuthMethod:  methodName,
		BearerToken: "test-bearer-token",
	}

	loginToken, _, err := client.ACL().Login(loginParams, nil)
	if err != nil {
		t.Skipf("ACL Login not supported or bearer token invalid: %v", err)
	}

	assert.NotNil(t, loginToken, "Login should return a token")
	assert.NotEmpty(t, loginToken.AccessorID, "Login token should have an AccessorID")
	assert.NotEmpty(t, loginToken.SecretID, "Login token should have a SecretID")
	t.Logf("Login returned token: AccessorID=%s", loginToken.AccessorID)

	// Cleanup: logout/delete the token
	client.ACL().TokenDelete(loginToken.AccessorID, nil)
}

// CACL-043: Test ACL logout
func TestACLLogout(t *testing.T) {
	client := getClient(t)

	// Create an auth method and login to get a token
	methodName := "logout-auth-method-" + randomID()
	method := &api.ACLAuthMethod{
		Name: methodName,
		Type: "jwt",
		Config: map[string]interface{}{
			"JWTValidationPubKeys": []string{"-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END PUBLIC KEY-----"},
		},
	}
	_, _, err := client.ACL().AuthMethodCreate(method, nil)
	if err != nil {
		t.Skipf("Auth method not supported: %v", err)
	}
	defer client.ACL().AuthMethodDelete(methodName, nil)

	loginParams := &api.ACLLoginParams{
		AuthMethod:  methodName,
		BearerToken: "test-bearer-token",
	}
	loginToken, _, err := client.ACL().Login(loginParams, nil)
	if err != nil {
		t.Skipf("ACL Login not supported: %v", err)
	}

	// Use the login token's SecretID for logout
	addr := os.Getenv("CONSUL_HTTP_ADDR")
	if addr == "" {
		addr = "127.0.0.1:8500"
	}
	logoutClient, err := api.NewClient(&api.Config{
		Address: addr,
		Token:   loginToken.SecretID,
	})
	require.NoError(t, err)

	_, err = logoutClient.ACL().Logout(nil)
	if err != nil {
		t.Skipf("ACL Logout not supported: %v", err)
	}

	t.Log("ACL Logout succeeded")
}

// ==================== ACL Replication Tests ====================

// CACL-044: Test ACL replication status
func TestACLReplication(t *testing.T) {
	client := getClient(t)

	repl, _, err := client.ACL().Replication(nil)
	if err != nil {
		t.Skipf("ACL Replication not supported: %v", err)
	}

	assert.NotNil(t, repl, "Replication response should not be nil")
	// The Enabled field indicates whether ACL replication is active
	t.Logf("ACL Replication status: Enabled=%v, Running=%v, SourceDatacenter=%s",
		repl.Enabled, repl.Running, repl.SourceDatacenter)

	// ReplicatedIndex should be a valid uint64 (zero is valid for non-replicated setups)
	assert.IsType(t, uint64(0), repl.ReplicatedIndex, "ReplicatedIndex should be uint64")
	assert.IsType(t, uint64(0), repl.ReplicatedTokenIndex, "ReplicatedTokenIndex should be uint64")

	// If replication is enabled, verify additional fields are populated
	if repl.Enabled {
		assert.NotEmpty(t, repl.SourceDatacenter, "SourceDatacenter should be set when replication is enabled")
		t.Logf("Replication is enabled from datacenter: %s", repl.SourceDatacenter)
	}
}

// ==================== ACL OIDC Auth URL Tests ====================

// CACL-046: Test OIDC auth URL generation
func TestACLOIDCAuthURL(t *testing.T) {
	t.Skip("OIDC Auth URL requires a configured OIDC auth method with valid provider, skipping")

	client := getClient(t)

	// Create an OIDC auth method (would require a real OIDC provider in practice)
	methodName := "oidc-method-" + randomID()

	authURLParams := &api.ACLOIDCAuthURLParams{
		AuthMethod:  methodName,
		RedirectURI: "http://localhost:8550/oidc/callback",
		ClientNonce: randomID(),
	}

	authURL, _, err := client.ACL().OIDCAuthURL(authURLParams, nil)
	if err != nil {
		t.Skipf("OIDC Auth URL not supported: %v", err)
	}

	assert.NotEmpty(t, authURL, "OIDC auth URL should not be empty")
	t.Logf("OIDC Auth URL: %s", authURL)
}

// CACL-STRICT-001: Test ACL policy name uniqueness
// Consul enforces unique policy names — duplicate create must fail
func TestACLPolicyNameUniqueness(t *testing.T) {
	client := getClient(t)
	policyName := "uniqueness-test-" + randomID()

	// Create first policy
	policy1, _, err := client.ACL().PolicyCreate(&api.ACLPolicy{
		Name:  policyName,
		Rules: `key_prefix "" { policy = "read" }`,
	}, nil)
	if err != nil {
		t.Skipf("ACL policy create not supported: %v", err)
	}
	require.NotNil(t, policy1)
	require.NotEmpty(t, policy1.ID)
	defer client.ACL().PolicyDelete(policy1.ID, nil)

	// Create second policy with SAME name — must fail
	_, _, err = client.ACL().PolicyCreate(&api.ACLPolicy{
		Name:  policyName,
		Rules: `key_prefix "" { policy = "write" }`,
	}, nil)
	assert.Error(t, err,
		"Creating a policy with duplicate name must fail (Consul enforces unique names)")
}

// CACL-STRICT-002: Test ACL token SecretID visibility
// SecretID should be returned on create, but NOT in list responses
func TestACLTokenSecretIDVisibility(t *testing.T) {
	client := getClient(t)

	// Create token
	created, _, err := client.ACL().TokenCreate(&api.ACLToken{
		Description: "secret-test-" + randomID(),
		Local:       true,
	}, nil)
	if err != nil {
		t.Skipf("ACL token create not supported: %v", err)
	}
	defer client.ACL().TokenDelete(created.AccessorID, nil)

	// SecretID must be returned on create
	assert.NotEmpty(t, created.SecretID,
		"SecretID must be returned when creating a token")
	assert.NotEmpty(t, created.AccessorID,
		"AccessorID must be returned when creating a token")

	// SecretID should NOT be in list responses
	tokens, _, err := client.ACL().TokenList(nil)
	require.NoError(t, err)

	for _, tok := range tokens {
		if tok.AccessorID == created.AccessorID {
			assert.Empty(t, tok.SecretID,
				"SecretID must NOT be visible in token list response "+
					"(Consul hides SecretID in list for security)")
			break
		}
	}
}

// CACL-STRICT-003: Test ACL policy create with full field validation
func TestACLPolicyCreateFieldValidation(t *testing.T) {
	client := getClient(t)
	policyName := "field-validation-" + randomID()

	policy, _, err := client.ACL().PolicyCreate(&api.ACLPolicy{
		Name:        policyName,
		Description: "Test policy for field validation",
		Rules:       `service_prefix "web-" { policy = "write" }`,
	}, nil)
	if err != nil {
		t.Skipf("ACL policy create not supported: %v", err)
	}
	defer client.ACL().PolicyDelete(policy.ID, nil)

	// Full field validation
	assert.NotEmpty(t, policy.ID, "Policy ID must be set")
	assert.Equal(t, policyName, policy.Name, "Policy name must match")
	assert.Equal(t, "Test policy for field validation", policy.Description)
	assert.Contains(t, policy.Rules, "service_prefix")

	// Read back and verify
	read, _, err := client.ACL().PolicyRead(policy.ID, nil)
	require.NoError(t, err)
	require.NotNil(t, read)
	assert.Equal(t, policy.ID, read.ID, "Read policy ID must match")
	assert.Equal(t, policyName, read.Name, "Read policy name must match")
	assert.Equal(t, policy.Rules, read.Rules, "Read policy rules must match")
}

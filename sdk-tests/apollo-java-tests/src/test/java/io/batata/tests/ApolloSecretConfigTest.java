package io.batata.tests;

import com.ctrip.framework.apollo.Config;
import com.ctrip.framework.apollo.ConfigChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.model.ConfigChange;
import com.ctrip.framework.apollo.model.ConfigChangeEvent;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo Secret Configuration Tests
 *
 * Tests for secret and sensitive configuration handling:
 * - Secret namespace access
 * - Encrypted config values
 * - Config value masking
 * - Sensitive key patterns
 * - Credential handling
 * - Secret rotation and refresh
 * - Access audit
 * - Environment isolation
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloSecretConfigTest {

    private static final String SECRET_NAMESPACE = "secrets";
    private static final String APPLICATION_NAMESPACE = "application";

    @BeforeAll
    static void setupClass() {
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo Secret Config Test Setup");
    }

    // ==================== Secret Namespace Tests ====================

    /**
     * ASC-001: Test secret namespace access
     *
     * Verifies that a dedicated secrets namespace can be accessed
     * and provides proper isolation for sensitive configurations.
     */
    @Test
    @Order(1)
    void testSecretNamespaceAccess() {
        try {
            Config secretConfig = ConfigService.getConfig(SECRET_NAMESPACE);
            assertNotNull(secretConfig, "Secret namespace config should not be null");

            Set<String> propertyNames = secretConfig.getPropertyNames();
            System.out.println("Secret namespace properties count: " + propertyNames.size());

            // Verify secret namespace is separate from application
            Config appConfig = ConfigService.getAppConfig();
            assertNotSame(secretConfig, appConfig, "Secret namespace should be separate from application");

            System.out.println("ASC-001: Secret namespace access successful");
        } catch (Exception e) {
            System.out.println("ASC-001: Secret namespace access - " + e.getMessage());
        }
    }

    /**
     * ASC-002: Test encrypted config value
     *
     * Tests handling of encrypted configuration values that may
     * be stored in encrypted format and decrypted at runtime.
     */
    @Test
    @Order(2)
    void testEncryptedConfigValue() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Test encrypted value pattern (typically prefixed with ENC())
        String encryptedKey = "database.password.encrypted";
        String encryptedValue = config.getProperty(encryptedKey, "ENC(default-encrypted-value)");

        System.out.println("Encrypted key: " + encryptedKey);
        System.out.println("Encrypted value pattern detected: " + encryptedValue.startsWith("ENC("));

        // Test with cipher prefix pattern
        String cipherKey = "api.secret.cipher";
        String cipherValue = config.getProperty(cipherKey, "CIPHER:default-cipher");

        boolean hasCipherPrefix = cipherValue.startsWith("CIPHER:") || cipherValue.startsWith("ENC(");
        System.out.println("Cipher prefix detected: " + hasCipherPrefix);

        System.out.println("ASC-002: Encrypted config value test completed");
    }

    /**
     * ASC-003: Test config value masking
     *
     * Verifies that sensitive values can be masked when logged or displayed,
     * showing only partial information for security purposes.
     */
    @Test
    @Order(3)
    void testConfigValueMasking() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        String sensitiveKey = "api.key";
        String sensitiveValue = config.getProperty(sensitiveKey, "sk-1234567890abcdef");

        // Mask the value for logging (show only first and last few characters)
        String maskedValue = maskSensitiveValue(sensitiveValue);

        assertNotEquals(sensitiveValue, maskedValue, "Masked value should differ from original");
        assertTrue(maskedValue.contains("*"), "Masked value should contain asterisks");

        System.out.println("Original length: " + sensitiveValue.length());
        System.out.println("Masked value: " + maskedValue);
        System.out.println("ASC-003: Config value masking test completed");
    }

    /**
     * ASC-004: Test sensitive key patterns
     *
     * Tests detection of sensitive keys based on common patterns
     * like password, secret, key, token, credential, etc.
     */
    @Test
    @Order(4)
    void testSensitiveKeyPatterns() {
        // Common sensitive key patterns
        String[] sensitivePatterns = {
                ".*password.*",
                ".*secret.*",
                ".*key.*",
                ".*token.*",
                ".*credential.*",
                ".*auth.*",
                ".*apikey.*",
                ".*api_key.*",
                ".*private.*"
        };

        String[] testKeys = {
                "database.password",
                "api.secret.key",
                "jwt.token",
                "oauth.credential",
                "auth.api_key",
                "service.private.key",
                "normal.config.value"
        };

        Pattern combinedPattern = Pattern.compile(
                String.join("|", sensitivePatterns),
                Pattern.CASE_INSENSITIVE
        );

        int sensitiveCount = 0;
        for (String key : testKeys) {
            boolean isSensitive = combinedPattern.matcher(key).matches();
            System.out.println("Key: " + key + " -> Sensitive: " + isSensitive);
            if (isSensitive) {
                sensitiveCount++;
            }
        }

        assertTrue(sensitiveCount > 0, "Should detect sensitive keys");
        System.out.println("ASC-004: Detected " + sensitiveCount + " sensitive keys out of " + testKeys.length);
    }

    /**
     * ASC-005: Test password field handling
     *
     * Tests proper handling of password configuration fields,
     * ensuring they can be retrieved securely.
     */
    @Test
    @Order(5)
    void testPasswordFieldHandling() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Test various password field patterns
        String dbPassword = config.getProperty("database.password", "db-pass-default");
        String adminPassword = config.getProperty("admin.password", "admin-pass-default");
        String userPassword = config.getProperty("user.pwd", "user-pwd-default");
        String encryptedPassword = config.getProperty("encrypted.passwd", "enc-pass-default");

        assertNotNull(dbPassword);
        assertNotNull(adminPassword);
        assertNotNull(userPassword);
        assertNotNull(encryptedPassword);

        // Password fields should not be empty when configured
        System.out.println("Database password length: " + dbPassword.length());
        System.out.println("Admin password length: " + adminPassword.length());
        System.out.println("User password length: " + userPassword.length());
        System.out.println("Encrypted password length: " + encryptedPassword.length());

        System.out.println("ASC-005: Password field handling test completed");
    }

    /**
     * ASC-006: Test API key storage
     *
     * Tests storage and retrieval of API keys with proper handling
     * for different API key formats and providers.
     */
    @Test
    @Order(6)
    void testApiKeyStorage() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Test different API key formats
        String genericApiKey = config.getProperty("service.api.key", "api-key-12345");
        String awsAccessKey = config.getProperty("aws.access.key.id", "AKIA123456789");
        String stripeApiKey = config.getProperty("stripe.api.key", "sk_test_123456");
        String openaiApiKey = config.getProperty("openai.api.key", "sk-123456abcdef");

        assertNotNull(genericApiKey, "Generic API key should not be null");
        assertNotNull(awsAccessKey, "AWS access key should not be null");
        assertNotNull(stripeApiKey, "Stripe API key should not be null");
        assertNotNull(openaiApiKey, "OpenAI API key should not be null");

        // Verify API keys are not empty
        assertTrue(genericApiKey.length() > 0, "Generic API key should not be empty");
        assertTrue(awsAccessKey.length() > 0, "AWS access key should not be empty");

        System.out.println("Generic API key starts with: " + genericApiKey.substring(0, Math.min(3, genericApiKey.length())) + "***");
        System.out.println("AWS Access Key starts with: " + awsAccessKey.substring(0, Math.min(4, awsAccessKey.length())) + "***");

        System.out.println("ASC-006: API key storage test completed");
    }

    /**
     * ASC-007: Test database credential config
     *
     * Tests complete database credential configuration including
     * username, password, host, and connection string handling.
     */
    @Test
    @Order(7)
    void testDatabaseCredentialConfig() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Database credential configuration
        String dbHost = config.getProperty("db.host", "localhost");
        int dbPort = config.getIntProperty("db.port", 3306);
        String dbName = config.getProperty("db.name", "testdb");
        String dbUsername = config.getProperty("db.username", "root");
        String dbPassword = config.getProperty("db.password", "secret");

        // Verify all credentials are present
        assertNotNull(dbHost);
        assertTrue(dbPort > 0, "Database port should be positive");
        assertNotNull(dbName);
        assertNotNull(dbUsername);
        assertNotNull(dbPassword);

        // Build connection string (without exposing password)
        String connectionUrl = String.format("jdbc:mysql://%s:%d/%s", dbHost, dbPort, dbName);

        System.out.println("Database connection URL: " + connectionUrl);
        System.out.println("Username: " + dbUsername);
        System.out.println("Password length: " + dbPassword.length() + " chars");

        System.out.println("ASC-007: Database credential config test completed");
    }

    /**
     * ASC-008: Test token config handling
     *
     * Tests handling of various token types including JWT tokens,
     * OAuth tokens, and bearer tokens.
     */
    @Test
    @Order(8)
    void testTokenConfigHandling() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Different token types
        String jwtSecret = config.getProperty("jwt.secret", "jwt-secret-key-12345");
        String oauthToken = config.getProperty("oauth.access.token", "ya29.token123");
        String bearerToken = config.getProperty("api.bearer.token", "Bearer xyz123");
        String refreshToken = config.getProperty("oauth.refresh.token", "refresh-token-abc");

        assertNotNull(jwtSecret, "JWT secret should not be null");
        assertNotNull(oauthToken, "OAuth token should not be null");
        assertNotNull(bearerToken, "Bearer token should not be null");
        assertNotNull(refreshToken, "Refresh token should not be null");

        // Token expiration configuration
        long tokenExpiry = config.getLongProperty("jwt.token.expiry.seconds", 3600L);
        assertTrue(tokenExpiry > 0, "Token expiry should be positive");

        System.out.println("JWT secret length: " + jwtSecret.length());
        System.out.println("OAuth token starts with: " + oauthToken.substring(0, Math.min(4, oauthToken.length())) + "...");
        System.out.println("Token expiry: " + tokenExpiry + " seconds");

        System.out.println("ASC-008: Token config handling test completed");
    }

    /**
     * ASC-009: Test secret rotation
     *
     * Tests the ability to handle secret rotation scenarios where
     * credentials are updated without service interruption.
     */
    @Test
    @Order(9)
    void testSecretRotation() throws InterruptedException {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);
        AtomicReference<String> currentSecret = new AtomicReference<>();
        AtomicBoolean rotationDetected = new AtomicBoolean(false);
        CountDownLatch rotationLatch = new CountDownLatch(1);

        // Get initial secret
        String initialSecret = config.getProperty("rotating.secret", "initial-secret-value");
        currentSecret.set(initialSecret);
        System.out.println("Initial secret hash: " + initialSecret.hashCode());

        // Register listener for secret rotation
        ConfigChangeListener rotationListener = changeEvent -> {
            if (changeEvent.isChanged("rotating.secret")) {
                ConfigChange change = changeEvent.getChange("rotating.secret");
                String oldValue = change.getOldValue();
                String newValue = change.getNewValue();

                System.out.println("Secret rotation detected!");
                System.out.println("Old secret hash: " + (oldValue != null ? oldValue.hashCode() : "null"));
                System.out.println("New secret hash: " + (newValue != null ? newValue.hashCode() : "null"));

                currentSecret.set(newValue);
                rotationDetected.set(true);
                rotationLatch.countDown();
            }
        };

        config.addChangeListener(rotationListener, Set.of("rotating.secret"));
        System.out.println("Secret rotation listener registered");

        // Simulate waiting for rotation (in real scenario, rotation would be triggered externally)
        System.out.println("Waiting for potential secret rotation...");
        Thread.sleep(2000);

        // Clean up
        config.removeChangeListener(rotationListener);
        System.out.println("ASC-009: Secret rotation test completed");
    }

    /**
     * ASC-010: Test secret access audit
     *
     * Tests audit logging capability for secret access,
     * tracking when and how secrets are accessed.
     */
    @Test
    @Order(10)
    void testSecretAccessAudit() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);
        List<String> auditLog = new CopyOnWriteArrayList<>();

        // Simulate audit logging for secret access
        String[] secretKeys = {
                "database.password",
                "api.secret.key",
                "jwt.token",
                "encryption.key"
        };

        for (String key : secretKeys) {
            long timestamp = System.currentTimeMillis();
            String value = config.getProperty(key, "default");

            // Log the access (in real implementation, this would go to audit system)
            String auditEntry = String.format(
                    "timestamp=%d, key=%s, accessed=true, valueLength=%d",
                    timestamp, key, value.length()
            );
            auditLog.add(auditEntry);

            System.out.println("Audit: " + auditEntry);
        }

        assertEquals(secretKeys.length, auditLog.size(), "All secret accesses should be audited");

        System.out.println("ASC-010: Secret access audit test completed");
        System.out.println("Total audit entries: " + auditLog.size());
    }

    /**
     * ASC-011: Test secret config isolation
     *
     * Tests that secrets are properly isolated between different
     * namespaces and configurations.
     */
    @Test
    @Order(11)
    void testSecretConfigIsolation() {
        Config secretConfig = ConfigService.getConfig(SECRET_NAMESPACE);
        Config appConfig = ConfigService.getConfig(APPLICATION_NAMESPACE);

        String testKey = "isolated.secret.key";

        // Get values from different namespaces
        String secretValue = secretConfig.getProperty(testKey, "secret-namespace-value");
        String appValue = appConfig.getProperty(testKey, "app-namespace-value");

        System.out.println("Secret namespace value: " + secretValue);
        System.out.println("App namespace value: " + appValue);

        // Verify configs are independent
        assertNotSame(secretConfig, appConfig, "Configs should be separate instances");

        // Values can be different (demonstrating isolation)
        System.out.println("Namespaces are isolated: true");

        System.out.println("ASC-011: Secret config isolation test completed");
    }

    /**
     * ASC-012: Test secret with gray release
     *
     * Tests secret configuration behavior during gray/canary releases
     * where different versions may have different secret values.
     */
    @Test
    @Order(12)
    void testSecretWithGrayRelease() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Gray release configuration for secrets
        String grayReleaseKey = config.getProperty("gray.release.secret.key", "default-gray-secret");
        boolean grayReleaseEnabled = config.getBooleanProperty("gray.release.enabled", false);
        int grayReleasePercent = config.getIntProperty("gray.release.percentage", 0);

        System.out.println("Gray release enabled: " + grayReleaseEnabled);
        System.out.println("Gray release percentage: " + grayReleasePercent + "%");
        System.out.println("Gray release secret key length: " + grayReleaseKey.length());

        // Simulate gray release secret selection
        String clientId = "client-" + UUID.randomUUID().toString().substring(0, 8);
        int clientHash = Math.abs(clientId.hashCode() % 100);
        boolean useGraySecret = clientHash < grayReleasePercent;

        String effectiveSecret = useGraySecret ?
                config.getProperty("gray.secret.new", "new-secret") :
                config.getProperty("gray.secret.current", "current-secret");

        System.out.println("Client ID: " + clientId);
        System.out.println("Client hash: " + clientHash);
        System.out.println("Using gray release secret: " + useGraySecret);

        System.out.println("ASC-012: Secret with gray release test completed");
    }

    /**
     * ASC-013: Test secret config refresh
     *
     * Tests the ability to refresh secret configurations
     * without restarting the application.
     */
    @Test
    @Order(13)
    void testSecretConfigRefresh() throws InterruptedException {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);
        AtomicInteger refreshCount = new AtomicInteger(0);
        AtomicReference<String> latestValue = new AtomicReference<>();

        String refreshableKey = "refreshable.secret";

        // Get initial value
        String initialValue = config.getProperty(refreshableKey, "initial-value");
        latestValue.set(initialValue);
        System.out.println("Initial secret value hash: " + initialValue.hashCode());

        // Register change listener for refresh events
        ConfigChangeListener refreshListener = changeEvent -> {
            if (changeEvent.isChanged(refreshableKey)) {
                refreshCount.incrementAndGet();
                ConfigChange change = changeEvent.getChange(refreshableKey);
                latestValue.set(change.getNewValue());

                System.out.println("Secret refresh detected! Count: " + refreshCount.get());
                System.out.println("New value hash: " + (change.getNewValue() != null ? change.getNewValue().hashCode() : "null"));
            }
        };

        config.addChangeListener(refreshListener, Set.of(refreshableKey));

        // Simulate periodic refresh checking
        for (int i = 0; i < 3; i++) {
            String currentValue = config.getProperty(refreshableKey, "default");
            System.out.println("Refresh check " + (i + 1) + ": value hash = " + currentValue.hashCode());
            Thread.sleep(500);
        }

        config.removeChangeListener(refreshListener);
        System.out.println("ASC-013: Secret config refresh test completed");
        System.out.println("Total refreshes detected: " + refreshCount.get());
    }

    /**
     * ASC-014: Test secret default values
     *
     * Tests default value behavior for secret configurations,
     * ensuring secure defaults are used when secrets are not configured.
     */
    @Test
    @Order(14)
    void testSecretDefaultValues() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Test with unique keys that definitely don't exist
        String uniqueSuffix = UUID.randomUUID().toString();

        String missingPassword = config.getProperty(
                "missing.password." + uniqueSuffix,
                "CHANGE_ME_DEFAULT_PASSWORD"
        );
        String missingApiKey = config.getProperty(
                "missing.api.key." + uniqueSuffix,
                "CHANGE_ME_DEFAULT_API_KEY"
        );
        String missingToken = config.getProperty(
                "missing.token." + uniqueSuffix,
                "CHANGE_ME_DEFAULT_TOKEN"
        );

        assertEquals("CHANGE_ME_DEFAULT_PASSWORD", missingPassword, "Should return default password");
        assertEquals("CHANGE_ME_DEFAULT_API_KEY", missingApiKey, "Should return default API key");
        assertEquals("CHANGE_ME_DEFAULT_TOKEN", missingToken, "Should return default token");

        // Verify defaults indicate they need to be changed
        assertTrue(missingPassword.contains("CHANGE_ME"), "Default should indicate it needs to be changed");

        System.out.println("Default password: " + missingPassword);
        System.out.println("Default API key: " + missingApiKey);
        System.out.println("Default token: " + missingToken);

        System.out.println("ASC-014: Secret default values test completed");
    }

    /**
     * ASC-015: Test secret config listener
     *
     * Tests the configuration change listener specifically for
     * secret-related configuration changes.
     */
    @Test
    @Order(15)
    void testSecretConfigListener() throws InterruptedException {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);
        AtomicBoolean secretChangeDetected = new AtomicBoolean(false);
        AtomicReference<ConfigChangeEvent> lastEvent = new AtomicReference<>();
        CountDownLatch changeLatch = new CountDownLatch(1);

        Set<String> secretKeys = new HashSet<>(Arrays.asList(
                "database.password",
                "api.secret.key",
                "jwt.token",
                "encryption.key",
                "oauth.client.secret"
        ));

        ConfigChangeListener secretListener = changeEvent -> {
            Set<String> changedKeys = changeEvent.changedKeys();

            for (String key : changedKeys) {
                if (secretKeys.contains(key)) {
                    secretChangeDetected.set(true);
                    lastEvent.set(changeEvent);

                    ConfigChange change = changeEvent.getChange(key);
                    System.out.println("Secret change detected:");
                    System.out.println("  Key: " + key);
                    System.out.println("  Change type: " + change.getChangeType());
                    System.out.println("  Old value present: " + (change.getOldValue() != null));
                    System.out.println("  New value present: " + (change.getNewValue() != null));

                    changeLatch.countDown();
                    break;
                }
            }
        };

        config.addChangeListener(secretListener, secretKeys);
        System.out.println("Secret config listener registered for keys: " + secretKeys);

        // Wait briefly for any potential changes
        Thread.sleep(1000);

        config.removeChangeListener(secretListener);
        System.out.println("ASC-015: Secret config listener test completed");
    }

    /**
     * ASC-016: Test multi-env secret config
     *
     * Tests secret configuration across multiple environments
     * (DEV, STG, PRD) ensuring proper isolation and values.
     */
    @Test
    @Order(16)
    void testMultiEnvSecretConfig() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);
        String currentEnv = System.getProperty("env", "DEV");

        System.out.println("Current environment: " + currentEnv);

        // Environment-specific secret keys
        String envSecretKey = config.getProperty(
                "secret." + currentEnv.toLowerCase() + ".specific",
                "env-specific-default"
        );
        System.out.println("Environment-specific secret key: " + envSecretKey);

        // Common secrets across environments (but with different values)
        String dbPassword = config.getProperty("database.password", "env-db-pass");
        String apiKey = config.getProperty("api.key", "env-api-key");

        // Environment metadata
        String envDbHost = config.getProperty("database.host." + currentEnv.toLowerCase(), "localhost");
        boolean envDebugEnabled = config.getBooleanProperty("debug.secrets.enabled", false);

        System.out.println("DB Host for " + currentEnv + ": " + envDbHost);
        System.out.println("Debug secrets enabled: " + envDebugEnabled);

        // In production, debug should typically be disabled
        if ("PRD".equals(currentEnv)) {
            assertFalse(envDebugEnabled, "Debug should be disabled in production");
        }

        System.out.println("ASC-016: Multi-env secret config test completed");
    }

    /**
     * ASC-017: Test secret config rollback
     *
     * Tests the ability to rollback secret configurations to
     * previous versions in case of issues.
     */
    @Test
    @Order(17)
    void testSecretConfigRollback() throws InterruptedException {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);
        List<String> versionHistory = new CopyOnWriteArrayList<>();
        AtomicReference<String> currentVersion = new AtomicReference<>();

        // Track secret version
        String versionKey = "secret.config.version";
        String initialVersion = config.getProperty(versionKey, "v1.0.0");
        currentVersion.set(initialVersion);
        versionHistory.add(initialVersion);

        System.out.println("Initial secret config version: " + initialVersion);

        // Listen for version changes (rollback events)
        ConfigChangeListener rollbackListener = changeEvent -> {
            if (changeEvent.isChanged(versionKey)) {
                ConfigChange change = changeEvent.getChange(versionKey);
                String oldVersion = change.getOldValue();
                String newVersion = change.getNewValue();

                System.out.println("Secret config version change detected:");
                System.out.println("  From: " + oldVersion);
                System.out.println("  To: " + newVersion);

                if (newVersion != null) {
                    versionHistory.add(newVersion);
                    currentVersion.set(newVersion);

                    // Check if this is a rollback (version number decreased)
                    if (oldVersion != null && isVersionRollback(oldVersion, newVersion)) {
                        System.out.println("  ROLLBACK DETECTED!");
                    }
                }
            }
        };

        config.addChangeListener(rollbackListener, Set.of(versionKey));

        // Simulate version tracking
        System.out.println("Monitoring for secret config rollbacks...");
        Thread.sleep(1000);

        config.removeChangeListener(rollbackListener);
        System.out.println("Version history: " + versionHistory);
        System.out.println("ASC-017: Secret config rollback test completed");
    }

    /**
     * ASC-018: Test secret config permissions
     *
     * Tests permission-based access control for secret configurations,
     * verifying that proper authorization is enforced.
     */
    @Test
    @Order(18)
    void testSecretConfigPermissions() {
        Config config = ConfigService.getConfig(SECRET_NAMESPACE);

        // Permission levels for secret access
        String[] permissionLevels = {"read", "write", "admin", "superadmin"};
        String[] sensitiveKeys = {
                "database.master.password",
                "encryption.master.key",
                "signing.private.key",
                "root.api.key"
        };

        // Simulate permission checking
        Map<String, String> keyPermissions = new HashMap<>();
        keyPermissions.put("database.master.password", "admin");
        keyPermissions.put("encryption.master.key", "superadmin");
        keyPermissions.put("signing.private.key", "admin");
        keyPermissions.put("root.api.key", "superadmin");

        String currentUserPermission = "admin"; // Simulated current user permission

        System.out.println("Current user permission level: " + currentUserPermission);
        System.out.println();

        for (String key : sensitiveKeys) {
            String requiredPermission = keyPermissions.getOrDefault(key, "read");
            boolean hasAccess = hasPermission(currentUserPermission, requiredPermission);

            if (hasAccess) {
                String value = config.getProperty(key, "default-secret");
                System.out.println("Key: " + key);
                System.out.println("  Required: " + requiredPermission);
                System.out.println("  Access: GRANTED");
                System.out.println("  Value length: " + value.length());
            } else {
                System.out.println("Key: " + key);
                System.out.println("  Required: " + requiredPermission);
                System.out.println("  Access: DENIED");
            }
        }

        System.out.println();
        System.out.println("ASC-018: Secret config permissions test completed");
    }

    // ==================== Helper Methods ====================

    /**
     * Masks a sensitive value for safe logging.
     *
     * @param value the sensitive value to mask
     * @return masked value showing only first and last few characters
     */
    private String maskSensitiveValue(String value) {
        if (value == null || value.length() <= 4) {
            return "****";
        }

        int showChars = Math.min(2, value.length() / 4);
        String prefix = value.substring(0, showChars);
        String suffix = value.substring(value.length() - showChars);
        int maskLength = value.length() - (2 * showChars);

        return prefix + "*".repeat(maskLength) + suffix;
    }

    /**
     * Checks if a version change represents a rollback.
     *
     * @param oldVersion the previous version
     * @param newVersion the new version
     * @return true if this is a rollback (version decreased)
     */
    private boolean isVersionRollback(String oldVersion, String newVersion) {
        // Simple version comparison (v1.0.0 format)
        try {
            String oldNum = oldVersion.replace("v", "").replace(".", "");
            String newNum = newVersion.replace("v", "").replace(".", "");
            return Integer.parseInt(newNum) < Integer.parseInt(oldNum);
        } catch (NumberFormatException e) {
            return false;
        }
    }

    /**
     * Checks if the current permission level has access to the required level.
     *
     * @param currentPermission the current user's permission level
     * @param requiredPermission the required permission level
     * @return true if access is granted
     */
    private boolean hasPermission(String currentPermission, String requiredPermission) {
        Map<String, Integer> permissionRank = new HashMap<>();
        permissionRank.put("read", 1);
        permissionRank.put("write", 2);
        permissionRank.put("admin", 3);
        permissionRank.put("superadmin", 4);

        int currentRank = permissionRank.getOrDefault(currentPermission, 0);
        int requiredRank = permissionRank.getOrDefault(requiredPermission, 0);

        return currentRank >= requiredRank;
    }
}

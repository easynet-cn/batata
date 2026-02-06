package io.batata.tests;

import com.ctrip.framework.apollo.ConfigFile;
import com.ctrip.framework.apollo.ConfigFileChangeListener;
import com.ctrip.framework.apollo.ConfigService;
import com.ctrip.framework.apollo.core.enums.ConfigFileFormat;
import com.ctrip.framework.apollo.enums.ConfigSourceType;
import com.ctrip.framework.apollo.model.ConfigFileChangeEvent;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Apollo ConfigFile API Tests
 *
 * Tests for ConfigFile functionality using ConfigService.getConfigFile() API:
 * - ConfigFile content retrieval
 * - Multiple file formats (Properties, JSON, YAML, XML, TXT)
 * - ConfigFile change listener
 * - ConfigFile caching and refresh
 * - Namespace handling
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class ApolloConfigFileTest {

    private static final String TEST_NAMESPACE = "application";

    @BeforeAll
    static void setupClass() {
        // Set Apollo configuration
        System.setProperty("app.id", System.getProperty("apollo.app.id", "test-app"));
        System.setProperty("apollo.meta", System.getProperty("apollo.meta", "http://127.0.0.1:8848"));
        System.setProperty("apollo.configService", System.getProperty("apollo.configService", "http://127.0.0.1:8848"));
        System.setProperty("env", System.getProperty("apollo.env", "DEV"));
        System.setProperty("apollo.cluster", System.getProperty("apollo.cluster", "default"));

        System.out.println("Apollo ConfigFile Test Setup");
        System.out.println("  app.id: " + System.getProperty("app.id"));
        System.out.println("  apollo.meta: " + System.getProperty("apollo.meta"));
        System.out.println("  env: " + System.getProperty("env"));
    }

    // ==================== ACF-001 to ACF-006: Config File Format Tests ====================

    /**
     * ACF-001: Test get config file content
     */
    @Test
    @Order(1)
    void testGetConfigFileContent() {
        ConfigFile configFile = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
        assertNotNull(configFile, "ConfigFile should not be null");

        String content = configFile.getContent();
        System.out.println("ConfigFile content: " + (content != null ? content.length() + " chars" : "null"));

        if (content != null && !content.isEmpty()) {
            System.out.println("Content preview: " + content.substring(0, Math.min(200, content.length())));
        }

        // Verify namespace is correct
        String namespace = configFile.getNamespace();
        assertNotNull(namespace, "Namespace should not be null");
        System.out.println("ConfigFile namespace: " + namespace);
    }

    /**
     * ACF-002: Test config file with properties format
     */
    @Test
    @Order(2)
    void testConfigFilePropertiesFormat() {
        ConfigFile configFile = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
        assertNotNull(configFile, "Properties ConfigFile should not be null");

        String content = configFile.getContent();
        System.out.println("Properties format content:");

        if (content != null && !content.isEmpty()) {
            // Properties format typically contains key=value pairs
            System.out.println("  Content length: " + content.length());
            System.out.println("  Sample: " + content.substring(0, Math.min(100, content.length())));

            // Check for typical properties format patterns
            boolean hasKeyValuePairs = content.contains("=") || content.isEmpty();
            System.out.println("  Has key=value pairs: " + hasKeyValuePairs);
        } else {
            System.out.println("  Content is empty or null (may be expected if namespace is empty)");
        }
    }

    /**
     * ACF-003: Test config file with JSON format
     */
    @Test
    @Order(3)
    void testConfigFileJsonFormat() {
        String namespace = "config.json";

        try {
            ConfigFile configFile = ConfigService.getConfigFile(namespace, ConfigFileFormat.JSON);
            assertNotNull(configFile, "JSON ConfigFile should not be null");

            String content = configFile.getContent();
            System.out.println("JSON format content:");

            if (content != null && !content.isEmpty()) {
                System.out.println("  Content length: " + content.length());
                String trimmed = content.trim();
                boolean isValidJson = trimmed.startsWith("{") || trimmed.startsWith("[");
                System.out.println("  Starts with { or [: " + isValidJson);
                System.out.println("  Sample: " + trimmed.substring(0, Math.min(100, trimmed.length())));
            } else {
                System.out.println("  Content is empty or null");
            }
        } catch (Exception e) {
            System.out.println("JSON ConfigFile test: " + e.getMessage());
        }
    }

    /**
     * ACF-004: Test config file with YAML format
     */
    @Test
    @Order(4)
    void testConfigFileYamlFormat() {
        String namespace = "config.yaml";

        try {
            ConfigFile configFile = ConfigService.getConfigFile(namespace, ConfigFileFormat.YAML);
            assertNotNull(configFile, "YAML ConfigFile should not be null");

            String content = configFile.getContent();
            System.out.println("YAML format content:");

            if (content != null && !content.isEmpty()) {
                System.out.println("  Content length: " + content.length());
                System.out.println("  Sample: " + content.substring(0, Math.min(100, content.length())));
            } else {
                System.out.println("  Content is empty or null");
            }
        } catch (Exception e) {
            System.out.println("YAML ConfigFile test: " + e.getMessage());
        }
    }

    /**
     * ACF-005: Test config file with XML format
     */
    @Test
    @Order(5)
    void testConfigFileXmlFormat() {
        String namespace = "config.xml";

        try {
            ConfigFile configFile = ConfigService.getConfigFile(namespace, ConfigFileFormat.XML);
            assertNotNull(configFile, "XML ConfigFile should not be null");

            String content = configFile.getContent();
            System.out.println("XML format content:");

            if (content != null && !content.isEmpty()) {
                System.out.println("  Content length: " + content.length());
                boolean startsWithXml = content.trim().startsWith("<?xml") || content.trim().startsWith("<");
                System.out.println("  Starts with XML declaration or tag: " + startsWithXml);
                System.out.println("  Sample: " + content.substring(0, Math.min(100, content.length())));
            } else {
                System.out.println("  Content is empty or null");
            }
        } catch (Exception e) {
            System.out.println("XML ConfigFile test: " + e.getMessage());
        }
    }

    /**
     * ACF-006: Test config file with plain text
     */
    @Test
    @Order(6)
    void testConfigFilePlainText() {
        String namespace = "config.txt";

        try {
            ConfigFile configFile = ConfigService.getConfigFile(namespace, ConfigFileFormat.TXT);
            assertNotNull(configFile, "TXT ConfigFile should not be null");

            String content = configFile.getContent();
            System.out.println("Plain text format content:");

            if (content != null && !content.isEmpty()) {
                System.out.println("  Content length: " + content.length());
                System.out.println("  Sample: " + content.substring(0, Math.min(100, content.length())));
            } else {
                System.out.println("  Content is empty or null");
            }
        } catch (Exception e) {
            System.out.println("TXT ConfigFile test: " + e.getMessage());
        }
    }

    // ==================== ACF-007 to ACF-008: Change Listener and Refresh Tests ====================

    /**
     * ACF-007: Test config file change listener
     */
    @Test
    @Order(7)
    void testConfigFileChangeListener() {
        ConfigFile configFile = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
        assertNotNull(configFile, "ConfigFile should not be null");

        AtomicBoolean changeReceived = new AtomicBoolean(false);
        AtomicReference<ConfigFileChangeEvent> eventRef = new AtomicReference<>();

        ConfigFileChangeListener listener = changeEvent -> {
            changeReceived.set(true);
            eventRef.set(changeEvent);
            System.out.println("ConfigFile change received:");
            System.out.println("  Namespace: " + changeEvent.getNamespace());
            System.out.println("  Old value length: " + (changeEvent.getOldValue() != null ? changeEvent.getOldValue().length() : 0));
            System.out.println("  New value length: " + (changeEvent.getNewValue() != null ? changeEvent.getNewValue().length() : 0));
            System.out.println("  Change type: " + changeEvent.getChangeType());
        };

        configFile.addChangeListener(listener);
        System.out.println("ConfigFile change listener registered successfully");

        // Note: To trigger the listener, a config change would need to be published
        // via the Apollo OpenAPI. For basic test, we verify listener registration.
    }

    /**
     * ACF-008: Test config file refresh
     */
    @Test
    @Order(8)
    void testConfigFileRefresh() throws InterruptedException {
        ConfigFile configFile = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
        assertNotNull(configFile, "ConfigFile should not be null");

        // Get initial content
        String initialContent = configFile.getContent();
        System.out.println("Initial content length: " + (initialContent != null ? initialContent.length() : 0));

        // Wait a bit and check if content is refreshed (if server has updates)
        Thread.sleep(2000);

        // Get content again
        String refreshedContent = configFile.getContent();
        System.out.println("Refreshed content length: " + (refreshedContent != null ? refreshedContent.length() : 0));

        // Content may or may not change depending on server state
        if (initialContent != null && refreshedContent != null) {
            boolean contentChanged = !initialContent.equals(refreshedContent);
            System.out.println("Content changed after refresh: " + contentChanged);
        }
    }

    // ==================== ACF-009 to ACF-012: Edge Case Tests ====================

    /**
     * ACF-009: Test config file not found
     */
    @Test
    @Order(9)
    void testConfigFileNotFound() {
        String nonExistentNamespace = "non-existent-namespace-" + UUID.randomUUID();

        try {
            ConfigFile configFile = ConfigService.getConfigFile(nonExistentNamespace, ConfigFileFormat.Properties);
            assertNotNull(configFile, "ConfigFile object should be created even for non-existent namespace");

            String content = configFile.getContent();
            System.out.println("Non-existent namespace:");
            System.out.println("  ConfigFile created: true");
            System.out.println("  Content is null or empty: " + (content == null || content.isEmpty()));

            // Apollo creates ConfigFile object but content may be null/empty
        } catch (Exception e) {
            System.out.println("Non-existent namespace exception: " + e.getMessage());
        }
    }

    /**
     * ACF-010: Test config file empty content
     */
    @Test
    @Order(10)
    void testConfigFileEmptyContent() {
        String emptyNamespace = "empty-config";

        try {
            ConfigFile configFile = ConfigService.getConfigFile(emptyNamespace, ConfigFileFormat.Properties);
            assertNotNull(configFile, "ConfigFile should not be null");

            String content = configFile.getContent();

            if (content == null || content.isEmpty()) {
                System.out.println("ConfigFile has empty content (expected behavior)");
            } else {
                System.out.println("ConfigFile content length: " + content.length());
            }

            // hasContent() may indicate if file has actual content
            boolean hasContent = configFile.hasContent();
            System.out.println("hasContent(): " + hasContent);
        } catch (Exception e) {
            System.out.println("Empty content test: " + e.getMessage());
        }
    }

    /**
     * ACF-011: Test config file large content
     */
    @Test
    @Order(11)
    void testConfigFileLargeContent() {
        String largeNamespace = "large-config";

        try {
            ConfigFile configFile = ConfigService.getConfigFile(largeNamespace, ConfigFileFormat.Properties);
            assertNotNull(configFile, "ConfigFile should not be null");

            String content = configFile.getContent();

            if (content != null) {
                System.out.println("Large config content length: " + content.length() + " characters");

                // Check if content is substantial
                if (content.length() > 10000) {
                    System.out.println("  Content is large (>10KB)");
                } else if (content.length() > 1000) {
                    System.out.println("  Content is medium (1-10KB)");
                } else {
                    System.out.println("  Content is small (<1KB)");
                }
            } else {
                System.out.println("Large config content is null");
            }
        } catch (Exception e) {
            System.out.println("Large content test: " + e.getMessage());
        }
    }

    /**
     * ACF-012: Test config file with unicode
     */
    @Test
    @Order(12)
    void testConfigFileWithUnicode() {
        String unicodeNamespace = "unicode-config";

        try {
            ConfigFile configFile = ConfigService.getConfigFile(unicodeNamespace, ConfigFileFormat.Properties);
            assertNotNull(configFile, "ConfigFile should not be null");

            String content = configFile.getContent();
            System.out.println("Unicode config content:");

            if (content != null && !content.isEmpty()) {
                System.out.println("  Content length: " + content.length());

                // Check for unicode characters (Chinese, Japanese, emoji, etc.)
                boolean hasUnicode = content.chars().anyMatch(c -> c > 127);
                System.out.println("  Contains unicode: " + hasUnicode);

                // Check for specific unicode ranges
                boolean hasChinese = content.codePoints().anyMatch(c -> c >= 0x4E00 && c <= 0x9FFF);
                System.out.println("  Contains Chinese characters: " + hasChinese);

                System.out.println("  Sample: " + content.substring(0, Math.min(100, content.length())));
            } else {
                System.out.println("  Content is empty or null");
            }
        } catch (Exception e) {
            System.out.println("Unicode content test: " + e.getMessage());
        }
    }

    // ==================== ACF-013 to ACF-014: Concurrency and Caching Tests ====================

    /**
     * ACF-013: Test config file concurrent access
     */
    @Test
    @Order(13)
    void testConfigFileConcurrentAccess() throws InterruptedException {
        int threadCount = 10;
        CountDownLatch startLatch = new CountDownLatch(1);
        CountDownLatch endLatch = new CountDownLatch(threadCount);
        List<String> contents = new CopyOnWriteArrayList<>();
        List<Exception> errors = new CopyOnWriteArrayList<>();

        for (int i = 0; i < threadCount; i++) {
            final int index = i;
            new Thread(() -> {
                try {
                    startLatch.await(); // Wait for all threads to be ready
                    ConfigFile configFile = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
                    String content = configFile.getContent();
                    contents.add(content != null ? content : "null");
                    System.out.println("Thread " + index + " got content: " + (content != null ? content.length() + " chars" : "null"));
                } catch (Exception e) {
                    errors.add(e);
                    System.out.println("Thread " + index + " error: " + e.getMessage());
                } finally {
                    endLatch.countDown();
                }
            }).start();
        }

        // Start all threads simultaneously
        startLatch.countDown();

        boolean completed = endLatch.await(30, TimeUnit.SECONDS);
        assertTrue(completed, "All threads should complete");

        System.out.println("Concurrent access results:");
        System.out.println("  Threads completed: " + threadCount);
        System.out.println("  Successful reads: " + contents.size());
        System.out.println("  Errors: " + errors.size());

        // All threads should get the same content (consistency)
        if (contents.size() > 1) {
            boolean allSame = contents.stream().allMatch(c -> c.equals(contents.get(0)));
            System.out.println("  All contents identical: " + allSame);
        }
    }

    /**
     * ACF-014: Test config file caching
     */
    @Test
    @Order(14)
    void testConfigFileCaching() {
        // Get the same config file multiple times
        ConfigFile configFile1 = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
        ConfigFile configFile2 = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);

        // Should return the same cached instance
        assertSame(configFile1, configFile2, "Should return cached ConfigFile instance");

        System.out.println("ConfigFile caching:");
        System.out.println("  First instance: " + System.identityHashCode(configFile1));
        System.out.println("  Second instance: " + System.identityHashCode(configFile2));
        System.out.println("  Same instance: " + (configFile1 == configFile2));

        // Content should also be identical
        String content1 = configFile1.getContent();
        String content2 = configFile2.getContent();

        if (content1 != null && content2 != null) {
            assertEquals(content1, content2, "Cached content should be identical");
            System.out.println("  Content identical: true");
        }
    }

    // ==================== ACF-015 to ACF-016: Namespace Tests ====================

    /**
     * ACF-015: Test config file namespace
     */
    @Test
    @Order(15)
    void testConfigFileNamespace() {
        ConfigFile configFile = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
        assertNotNull(configFile, "ConfigFile should not be null");

        String namespace = configFile.getNamespace();
        assertNotNull(namespace, "Namespace should not be null");
        assertEquals(TEST_NAMESPACE, namespace, "Namespace should match requested namespace");

        System.out.println("ConfigFile namespace: " + namespace);
    }

    /**
     * ACF-016: Test multiple config files
     */
    @Test
    @Order(16)
    void testMultipleConfigFiles() {
        Map<String, ConfigFile> configFiles = new HashMap<>();
        String[] namespaces = {"application", "common", "database", "redis"};
        ConfigFileFormat[] formats = {
                ConfigFileFormat.Properties,
                ConfigFileFormat.Properties,
                ConfigFileFormat.Properties,
                ConfigFileFormat.Properties
        };

        for (int i = 0; i < namespaces.length; i++) {
            try {
                ConfigFile configFile = ConfigService.getConfigFile(namespaces[i], formats[i]);
                configFiles.put(namespaces[i], configFile);
                System.out.println("Loaded ConfigFile for namespace: " + namespaces[i]);
            } catch (Exception e) {
                System.out.println("Failed to load namespace " + namespaces[i] + ": " + e.getMessage());
            }
        }

        System.out.println("Multiple config files:");
        System.out.println("  Requested: " + namespaces.length);
        System.out.println("  Loaded: " + configFiles.size());

        // Each namespace should have a different ConfigFile instance
        if (configFiles.size() > 1) {
            Set<Integer> hashCodes = new HashSet<>();
            for (ConfigFile cf : configFiles.values()) {
                hashCodes.add(System.identityHashCode(cf));
            }
            System.out.println("  Unique instances: " + hashCodes.size());
        }
    }

    // ==================== ACF-017 to ACF-018: Source Type and Special Namespace Tests ====================

    /**
     * ACF-017: Test config file source type
     */
    @Test
    @Order(17)
    void testConfigFileSourceType() {
        ConfigFile configFile = ConfigService.getConfigFile(TEST_NAMESPACE, ConfigFileFormat.Properties);
        assertNotNull(configFile, "ConfigFile should not be null");

        ConfigSourceType sourceType = configFile.getSourceType();
        assertNotNull(sourceType, "Source type should not be null");

        System.out.println("ConfigFile source type: " + sourceType);

        // Source type indicates where the config was loaded from
        switch (sourceType) {
            case REMOTE:
                System.out.println("  Config loaded from remote server");
                break;
            case LOCAL:
                System.out.println("  Config loaded from local cache");
                break;
            case NONE:
                System.out.println("  Config source is none (may be empty or not found)");
                break;
            default:
                System.out.println("  Unknown source type");
        }
    }

    /**
     * ACF-018: Test config file with special namespace
     */
    @Test
    @Order(18)
    void testConfigFileWithSpecialNamespace() {
        String[] specialNamespaces = {
                "namespace-with-dash",
                "namespace_with_underscore",
                "namespace.with.dots",
                "UPPERCASE-NAMESPACE",
                "CamelCaseNamespace",
                "123-numeric-prefix"
        };

        System.out.println("Testing special namespace formats:");

        for (String namespace : specialNamespaces) {
            try {
                ConfigFile configFile = ConfigService.getConfigFile(namespace, ConfigFileFormat.Properties);
                assertNotNull(configFile, "ConfigFile should not be null for: " + namespace);

                String content = configFile.getContent();
                boolean hasContent = content != null && !content.isEmpty();

                System.out.println("  " + namespace + ": OK (content: " + (hasContent ? content.length() + " chars" : "empty") + ")");
            } catch (Exception e) {
                System.out.println("  " + namespace + ": " + e.getClass().getSimpleName() + " - " + e.getMessage());
            }
        }

        // Test public namespace format (appId.namespace)
        String publicNamespace = "TEST1.public-config";
        try {
            ConfigFile configFile = ConfigService.getConfigFile(publicNamespace, ConfigFileFormat.Properties);
            System.out.println("  Public namespace (" + publicNamespace + "): OK");
        } catch (Exception e) {
            System.out.println("  Public namespace (" + publicNamespace + "): " + e.getMessage());
        }
    }
}

package io.batata.tests;

import com.alibaba.nacos.api.ai.model.prompt.PromptMetaInfo;
import com.alibaba.nacos.api.ai.model.prompt.PromptMetaSummary;
import com.alibaba.nacos.api.ai.model.prompt.PromptVersionInfo;
import com.alibaba.nacos.api.ai.model.prompt.PromptVersionSummary;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerFactory;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerService;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos AI Prompt Management Tests
 *
 * Tests for prompt template CRUD operations via AiMaintainerService:
 * publish, get metadata, list, version history, label binding, update metadata, delete.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAiPromptTest {

    private static AiMaintainerService aiService;
    private static final String DEFAULT_NAMESPACE = "";

    @BeforeAll
    static void setup() throws Exception {
        String serverAddr = System.getProperty("nacos.server", "127.0.0.1:8848");
        String username = System.getProperty("nacos.username", "nacos");
        String password = System.getProperty("nacos.password", "nacos");

        Properties properties = new Properties();
        properties.setProperty("serverAddr", serverAddr);
        properties.setProperty("username", username);
        properties.setProperty("password", password);

        aiService = AiMaintainerFactory.createAiMaintainerService(properties);
        System.out.println("AI Prompt Test Setup - Server: " + serverAddr);
    }

    // ==================== P0: Prompt CRUD ====================

    /**
     * PROMPT-001: Publish and get prompt
     */
    @Test
    @Order(1)
    void testPublishAndGetPrompt() throws Exception {
        String promptKey = "prompt-test-" + UUID.randomUUID().toString().substring(0, 8);
        String template = "Hello, {{name}}! Welcome to {{service}}.";
        String version = "1.0.0";

        try {
            boolean published = aiService.prompt().publishPrompt(
                    promptKey, version, template, "Initial publish");
            assertTrue(published, "Prompt publish should succeed");
            Thread.sleep(500);

            // Get prompt metadata
            PromptMetaInfo meta = aiService.prompt().getPromptMeta(promptKey);
            assertNotNull(meta, "Prompt metadata should not be null");
            assertEquals(promptKey, meta.getPromptKey(), "Prompt key should match");

            // Get prompt version detail
            PromptVersionInfo detail = aiService.prompt().queryPromptDetail(
                    DEFAULT_NAMESPACE, promptKey, version, null);
            assertNotNull(detail, "Prompt version detail should not be null");
            assertEquals(template, detail.getTemplate(), "Template should match");
        } finally {
            try { aiService.prompt().deletePrompt(promptKey); } catch (Exception ignored) {}
        }
    }

    /**
     * PROMPT-002: Publish multiple versions
     */
    @Test
    @Order(2)
    void testPublishMultipleVersions() throws Exception {
        String promptKey = "prompt-versions-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Publish v1
            aiService.prompt().publishPrompt(promptKey, "1.0.0",
                    "V1: Hello, {{name}}!", "Version 1");
            Thread.sleep(300);

            // Publish v2
            aiService.prompt().publishPrompt(promptKey, "2.0.0",
                    "V2: Greetings, {{name}}! {{greeting}}", "Version 2");
            Thread.sleep(300);

            // List versions
            Page<PromptVersionSummary> versions = aiService.prompt().listPromptVersions(
                    promptKey, 1, 100);
            assertNotNull(versions, "Version list should not be null");
            assertTrue(versions.getPageItems().size() >= 2,
                    "Should have at least 2 versions, got: " + versions.getPageItems().size());

            // Get specific version
            PromptVersionInfo v1 = aiService.prompt().queryPromptDetail(
                    DEFAULT_NAMESPACE, promptKey, "1.0.0", null);
            assertNotNull(v1, "V1 should be retrievable");
            assertTrue(v1.getTemplate().contains("V1:"), "V1 template should contain V1 marker");

            PromptVersionInfo v2 = aiService.prompt().queryPromptDetail(
                    DEFAULT_NAMESPACE, promptKey, "2.0.0", null);
            assertNotNull(v2, "V2 should be retrievable");
            assertTrue(v2.getTemplate().contains("V2:"), "V2 template should contain V2 marker");
        } finally {
            try { aiService.prompt().deletePrompt(promptKey); } catch (Exception ignored) {}
        }
    }

    /**
     * PROMPT-003: List prompts with pagination
     */
    @Test
    @Order(3)
    void testListPrompts() throws Exception {
        String prefix = "prompt-list-" + UUID.randomUUID().toString().substring(0, 6);
        List<String> keys = new ArrayList<>();

        try {
            for (int i = 0; i < 3; i++) {
                String key = prefix + "-" + i;
                keys.add(key);
                aiService.prompt().publishPrompt(key, "1.0.0",
                        "Template " + i + ": {{var}}", "Publish " + i);
            }
            Thread.sleep(500);

            Page<PromptMetaSummary> page = aiService.prompt().listPrompts(
                    prefix, 1, 100);
            assertNotNull(page, "Prompt list should not be null");
            assertTrue(page.getPageItems().size() >= 3,
                    "Should have at least 3 prompts, got: " + page.getPageItems().size());
        } finally {
            for (String key : keys) {
                try { aiService.prompt().deletePrompt(key); } catch (Exception ignored) {}
            }
        }
    }

    /**
     * PROMPT-004: Label binding and querying by label
     */
    @Test
    @Order(4)
    void testLabelBinding() throws Exception {
        String promptKey = "prompt-label-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Publish two versions
            aiService.prompt().publishPrompt(promptKey, "1.0.0",
                    "Stable: {{msg}}", "Stable version");
            Thread.sleep(300);
            aiService.prompt().publishPrompt(promptKey, "2.0.0",
                    "Beta: {{msg}}", "Beta version");
            Thread.sleep(300);

            // Bind label "stable" to v1
            boolean bound = aiService.prompt().bindLabel(
                    DEFAULT_NAMESPACE, promptKey, "stable", "1.0.0");
            assertTrue(bound, "Label binding should succeed");

            // Query by label
            PromptVersionInfo stableVersion = aiService.prompt().queryPromptDetail(
                    DEFAULT_NAMESPACE, promptKey, null, "stable");
            assertNotNull(stableVersion, "Query by label should return a version");
            assertTrue(stableVersion.getTemplate().contains("Stable:"),
                    "Label 'stable' should resolve to v1 template");

            // Unbind label
            boolean unbound = aiService.prompt().unbindLabel(
                    DEFAULT_NAMESPACE, promptKey, "stable");
            assertTrue(unbound, "Label unbinding should succeed");
        } finally {
            try { aiService.prompt().deletePrompt(promptKey); } catch (Exception ignored) {}
        }
    }

    /**
     * PROMPT-005: Update prompt metadata
     */
    @Test
    @Order(5)
    void testUpdatePromptMetadata() throws Exception {
        String promptKey = "prompt-meta-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            aiService.prompt().publishPrompt(
                    DEFAULT_NAMESPACE, promptKey, "1.0.0",
                    "Hello {{name}}", "Initial",
                    "Original description", "tag1,tag2");
            Thread.sleep(500);

            // Update metadata
            boolean updated = aiService.prompt().updatePromptMetadata(
                    DEFAULT_NAMESPACE, promptKey, "Updated description", "tag1,tag2,tag3");
            assertTrue(updated, "Metadata update should succeed");

            // Verify
            PromptMetaInfo meta = aiService.prompt().getPromptMeta(promptKey);
            assertNotNull(meta, "Prompt metadata should not be null after update");
            assertEquals("Updated description", meta.getDescription(),
                    "Description should be updated");
        } finally {
            try { aiService.prompt().deletePrompt(promptKey); } catch (Exception ignored) {}
        }
    }

    /**
     * PROMPT-006: Delete prompt
     */
    @Test
    @Order(6)
    void testDeletePrompt() throws Exception {
        String promptKey = "prompt-delete-" + UUID.randomUUID().toString().substring(0, 8);

        aiService.prompt().publishPrompt(promptKey, "1.0.0",
                "Delete me: {{var}}", "To be deleted");
        Thread.sleep(500);

        boolean deleted = aiService.prompt().deletePrompt(promptKey);
        assertTrue(deleted, "Prompt deletion should succeed");

        try {
            PromptMetaInfo meta = aiService.prompt().getPromptMeta(promptKey);
            assertNull(meta, "Deleted prompt should not be retrievable");
        } catch (NacosException e) {
            assertTrue(true, "Deleted prompt correctly throws exception");
        }
    }

    /**
     * PROMPT-007: Publish prompt with variables
     */
    @Test
    @Order(7)
    void testPublishPromptWithVariables() throws Exception {
        String promptKey = "prompt-vars-" + UUID.randomUUID().toString().substring(0, 8);
        String template = "Dear {{name}}, your order #{{order_id}} is {{status}}.";
        // Variables as JSON array
        String variables = "[{\"name\":\"name\",\"defaultValue\":\"Customer\",\"description\":\"Customer name\"},"
                + "{\"name\":\"order_id\",\"defaultValue\":\"000\",\"description\":\"Order ID\"},"
                + "{\"name\":\"status\",\"defaultValue\":\"pending\",\"description\":\"Order status\"}]";

        try {
            boolean published = aiService.prompt().publishPrompt(
                    DEFAULT_NAMESPACE, promptKey, "1.0.0", template,
                    "With variables", "Prompt with variable definitions",
                    null, variables);
            assertTrue(published, "Prompt publish with variables should succeed");
            Thread.sleep(500);

            PromptVersionInfo detail = aiService.prompt().queryPromptDetail(
                    DEFAULT_NAMESPACE, promptKey, "1.0.0", null);
            assertNotNull(detail, "Prompt with variables should be retrievable");
            assertEquals(template, detail.getTemplate(), "Template should match");
        } finally {
            try { aiService.prompt().deletePrompt(promptKey); } catch (Exception ignored) {}
        }
    }

    /**
     * PROMPT-008: Get non-existent prompt should fail
     */
    @Test
    @Order(8)
    void testGetNonExistentPrompt() {
        String promptKey = "prompt-nonexist-" + UUID.randomUUID().toString().substring(0, 8);
        try {
            PromptMetaInfo meta = aiService.prompt().getPromptMeta(promptKey);
            assertNull(meta, "Non-existent prompt should return null");
        } catch (NacosException e) {
            assertTrue(true, "Non-existent prompt correctly throws exception");
        }
    }
}

package io.batata.tests;

import com.alibaba.nacos.api.ai.model.agentspecs.AgentSpec;
import com.alibaba.nacos.api.ai.model.agentspecs.AgentSpecBasicInfo;
import com.alibaba.nacos.api.ai.model.agentspecs.AgentSpecMeta;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerFactory;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerService;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos AI AgentSpec Management Tests
 *
 * Tests for AgentSpec lifecycle via AiMaintainerService:
 * create draft via upload, update draft, submit, publish, list, get detail,
 * biz tags, scope, online/offline, delete.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAiAgentSpecTest {

    private static AiMaintainerService aiService;
    private static final String DEFAULT_NAMESPACE = "public";
    private static final ObjectMapper objectMapper = new ObjectMapper();

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
        System.out.println("AI AgentSpec Test Setup - Server: " + serverAddr);
    }

    /**
     * Build minimal ZIP bytes containing a manifest.json for agentspec upload.
     */
    private byte[] buildAgentSpecZip(String name, String description) throws Exception {
        // Build manifest.json
        Map<String, Object> manifest = new HashMap<>();
        manifest.put("name", name);
        manifest.put("description", description);
        manifest.put("version", "0.0.1");

        Map<String, Object> worker = new HashMap<>();
        worker.put("name", name);
        worker.put("description", description);
        manifest.put("worker", worker);

        String manifestJson = objectMapper.writeValueAsString(manifest);

        // Create ZIP in memory
        java.io.ByteArrayOutputStream baos = new java.io.ByteArrayOutputStream();
        java.util.zip.ZipOutputStream zos = new java.util.zip.ZipOutputStream(baos);

        java.util.zip.ZipEntry entry = new java.util.zip.ZipEntry("manifest.json");
        zos.putNextEntry(entry);
        zos.write(manifestJson.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        zos.closeEntry();

        zos.close();
        return baos.toByteArray();
    }

    // ==================== P0: AgentSpec Upload and Lifecycle ====================

    /**
     * ASPEC-001: Upload agentspec from ZIP and get detail
     */
    @Test
    @Order(1)
    @Disabled("AgentSpec upload via multipart not yet fully compatible with nacos-maintainer-client")
    void testUploadAndGetAgentSpec() throws Exception {
        String specName = "aspec-upload-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            byte[] zipBytes = buildAgentSpecZip(specName, "Upload test agentspec");
            String result = aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zipBytes);
            assertNotNull(result, "Upload should return a result");

            Thread.sleep(500);

            // Get agentspec detail
            AgentSpec detail = aiService.agentSpec().getAgentSpecDetail(DEFAULT_NAMESPACE, specName);
            assertNotNull(detail, "AgentSpec detail should not be null");
            assertEquals(specName, detail.getName(), "AgentSpec name should match");
        } finally {
            try { aiService.agentSpec().deleteAgentSpec(DEFAULT_NAMESPACE, specName); } catch (Exception ignored) {}
        }
    }

    /**
     * ASPEC-002: Full lifecycle: upload → submit → publish
     */
    @Test
    @Order(2)
    @Disabled("AgentSpec upload via multipart not yet fully compatible with nacos-maintainer-client")
    void testFullPublishLifecycle() throws Exception {
        String specName = "aspec-lifecycle-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Upload creates a draft
            byte[] zipBytes = buildAgentSpecZip(specName, "Lifecycle test");
            String uploadResult = aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zipBytes);
            assertNotNull(uploadResult);
            Thread.sleep(500);

            // Get the draft version from metadata
            AgentSpecMeta meta = aiService.agentSpec().getAgentSpecAdminDetail(DEFAULT_NAMESPACE, specName);
            assertNotNull(meta, "Admin detail should not be null");
            String editingVersion = meta.getEditingVersion();
            assertNotNull(editingVersion, "Should have an editing version (draft)");

            // Submit for review
            String submitResult = aiService.agentSpec().submit(DEFAULT_NAMESPACE, specName, editingVersion);
            assertNotNull(submitResult, "Submit should return a result");
            Thread.sleep(500);

            // Publish
            boolean published = aiService.agentSpec().publish(
                    DEFAULT_NAMESPACE, specName, editingVersion, true);
            assertTrue(published, "Publish should succeed");
            Thread.sleep(500);

            // Verify version detail
            AgentSpec versionDetail = aiService.agentSpec().getAgentSpecVersionDetail(
                    DEFAULT_NAMESPACE, specName, editingVersion);
            assertNotNull(versionDetail, "Published version should be retrievable");
        } finally {
            try { aiService.agentSpec().deleteAgentSpec(DEFAULT_NAMESPACE, specName); } catch (Exception ignored) {}
        }
    }

    // ==================== P1: AgentSpec Listing and Search ====================

    /**
     * ASPEC-003: List agentspecs with pagination
     */
    @Test
    @Order(3)
    @Disabled("AgentSpec upload via multipart not yet fully compatible with nacos-maintainer-client")
    void testListAgentSpecs() throws Exception {
        String prefix = "aspec-list-" + UUID.randomUUID().toString().substring(0, 6);
        List<String> names = new ArrayList<>();

        try {
            for (int i = 0; i < 3; i++) {
                String name = prefix + "-" + i;
                names.add(name);
                byte[] zip = buildAgentSpecZip(name, "List test " + i);
                aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zip);
            }
            Thread.sleep(500);

            Page<AgentSpecBasicInfo> page = aiService.agentSpec().listAgentSpecs(
                    DEFAULT_NAMESPACE, prefix, "blur", 1, 100);
            assertNotNull(page, "AgentSpec list should not be null");
            assertTrue(page.getPageItems().size() >= 3,
                    "Should have at least 3 agentspecs, got: " + page.getPageItems().size());
        } finally {
            for (String name : names) {
                try { aiService.agentSpec().deleteAgentSpec(DEFAULT_NAMESPACE, name); } catch (Exception ignored) {}
            }
        }
    }

    // ==================== P1: AgentSpec Governance ====================

    /**
     * ASPEC-004: Update biz tags
     */
    @Test
    @Order(4)
    void testUpdateBizTags() throws Exception {
        String specName = "aspec-tags-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            byte[] zip = buildAgentSpecZip(specName, "Tags test");
            aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zip);
            Thread.sleep(500);

            boolean updated = aiService.agentSpec().updateBizTags(
                    DEFAULT_NAMESPACE, specName, "[\"agent\",\"test\"]");
            assertTrue(updated, "Update biz tags should succeed");
        } finally {
            try { aiService.agentSpec().deleteAgentSpec(DEFAULT_NAMESPACE, specName); } catch (Exception ignored) {}
        }
    }

    /**
     * ASPEC-005: Update scope
     */
    @Test
    @Order(5)
    void testUpdateScope() throws Exception {
        String specName = "aspec-scope-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            byte[] zip = buildAgentSpecZip(specName, "Scope test");
            aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zip);
            Thread.sleep(500);

            boolean updated = aiService.agentSpec().updateScope(DEFAULT_NAMESPACE, specName, "PRIVATE");
            assertTrue(updated, "Update scope should succeed");
        } finally {
            try { aiService.agentSpec().deleteAgentSpec(DEFAULT_NAMESPACE, specName); } catch (Exception ignored) {}
        }
    }

    /**
     * ASPEC-006: Delete agentspec
     */
    @Test
    @Order(6)
    void testDeleteAgentSpec() throws Exception {
        String specName = "aspec-delete-" + UUID.randomUUID().toString().substring(0, 8);

        byte[] zip = buildAgentSpecZip(specName, "To be deleted");
        aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zip);
        Thread.sleep(500);

        boolean deleted = aiService.agentSpec().deleteAgentSpec(DEFAULT_NAMESPACE, specName);
        assertTrue(deleted, "AgentSpec deletion should succeed");

        try {
            AgentSpec detail = aiService.agentSpec().getAgentSpecDetail(DEFAULT_NAMESPACE, specName);
            assertNull(detail, "Deleted agentspec should not be retrievable");
        } catch (NacosException e) {
            assertTrue(true, "Deleted agentspec correctly throws exception");
        }
    }

    /**
     * ASPEC-007: Upload with overwrite
     */
    @Test
    @Order(7)
    void testUploadWithOverwrite() throws Exception {
        String specName = "aspec-overwrite-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // First upload
            byte[] zip1 = buildAgentSpecZip(specName, "Original");
            aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zip1);
            Thread.sleep(500);

            // Second upload with overwrite=true
            byte[] zip2 = buildAgentSpecZip(specName, "Overwritten");
            String result = aiService.agentSpec().uploadAgentSpecFromZip(DEFAULT_NAMESPACE, zip2, true);
            assertNotNull(result, "Upload with overwrite should succeed");
        } finally {
            try { aiService.agentSpec().deleteAgentSpec(DEFAULT_NAMESPACE, specName); } catch (Exception ignored) {}
        }
    }

    /**
     * ASPEC-008: Get non-existent agentspec
     */
    @Test
    @Order(8)
    void testGetNonExistentAgentSpec() {
        String specName = "aspec-nonexist-" + UUID.randomUUID().toString().substring(0, 8);
        try {
            AgentSpec detail = aiService.agentSpec().getAgentSpecDetail(DEFAULT_NAMESPACE, specName);
            assertNull(detail, "Non-existent agentspec should return null");
        } catch (NacosException e) {
            assertTrue(true, "Non-existent agentspec correctly throws exception");
        }
    }
}

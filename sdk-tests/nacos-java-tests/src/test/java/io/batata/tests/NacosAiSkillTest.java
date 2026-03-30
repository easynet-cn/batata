package io.batata.tests;

import com.alibaba.nacos.api.ai.model.skills.Skill;
import com.alibaba.nacos.api.ai.model.skills.SkillMeta;
import com.alibaba.nacos.api.ai.model.skills.SkillSummary;
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
 * Nacos AI Skill Management Tests
 *
 * Tests for Skill lifecycle via AiMaintainerService:
 * create draft, update draft, submit, publish, list, get metadata,
 * online/offline status, labels, biz tags, scope, delete.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAiSkillTest {

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
        System.out.println("AI Skill Test Setup - Server: " + serverAddr);
    }

    private String buildSkillCardJson(String name, String description) throws Exception {
        Map<String, Object> skillCard = new HashMap<>();
        skillCard.put("name", name);
        skillCard.put("description", description);
        skillCard.put("skillMd", "---\nname: " + name + "\ndescription: " + description
                + "\n---\n\n# " + name + "\n\nThis is a test skill.");
        return objectMapper.writeValueAsString(skillCard);
    }

    // ==================== P0: Skill Draft Lifecycle ====================

    /**
     * SKILL-001: Create skill draft and get metadata
     */
    @Test
    @Order(1)
    void testCreateDraftAndGetMeta() throws Exception {
        String skillName = "skill-draft-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            String skillCard = buildSkillCardJson(skillName, "Test skill draft");
            String draftVersion = aiService.skill().createDraft(DEFAULT_NAMESPACE, skillCard);
            assertNotNull(draftVersion, "Create draft should return a version string");
            assertFalse(draftVersion.isEmpty(), "Draft version should not be empty");

            // Get skill metadata
            SkillMeta meta = aiService.skill().getSkillMeta(DEFAULT_NAMESPACE, skillName);
            assertNotNull(meta, "Skill metadata should not be null");
            assertEquals(skillName, meta.getName(), "Skill name should match");
        } finally {
            try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
        }
    }

    /**
     * SKILL-002: Update skill draft
     */
    @Test
    @Order(2)
    void testUpdateDraft() throws Exception {
        String skillName = "skill-upd-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            String skillCard = buildSkillCardJson(skillName, "Original description");
            aiService.skill().createDraft(DEFAULT_NAMESPACE, skillCard);
            Thread.sleep(500);

            // Update draft with new description
            String updatedCard = buildSkillCardJson(skillName, "Updated description");
            boolean updated = aiService.skill().updateDraft(DEFAULT_NAMESPACE, updatedCard, null);
            assertTrue(updated, "Update draft should succeed");
        } finally {
            try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
        }
    }

    /**
     * SKILL-003: Delete skill draft
     */
    @Test
    @Order(3)
    void testDeleteDraft() throws Exception {
        String skillName = "skill-deldraft-" + UUID.randomUUID().toString().substring(0, 8);

        String skillCard = buildSkillCardJson(skillName, "To be deleted draft");
        String draftVersion = aiService.skill().createDraft(DEFAULT_NAMESPACE, skillCard);
        assertNotNull(draftVersion);

        boolean deleted = aiService.skill().deleteDraft(DEFAULT_NAMESPACE, skillName);
        assertTrue(deleted, "Delete draft should succeed");

        // Cleanup entire skill
        try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
    }

    // ==================== P0: Skill Publish Lifecycle ====================

    /**
     * SKILL-004: Full lifecycle: create draft → submit → publish
     */
    @Test
    @Order(4)
    void testFullPublishLifecycle() throws Exception {
        String skillName = "skill-lifecycle-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Step 1: Create draft
            String skillCard = buildSkillCardJson(skillName, "Full lifecycle test");
            String draftVersion = aiService.skill().createDraft(DEFAULT_NAMESPACE, skillCard);
            assertNotNull(draftVersion, "Draft should be created");
            Thread.sleep(500);

            // Step 2: Submit for review
            String submitResult = aiService.skill().submit(DEFAULT_NAMESPACE, skillName, draftVersion);
            assertNotNull(submitResult, "Submit should return a result");
            Thread.sleep(500);

            // Step 3: Publish
            boolean published = aiService.skill().publish(
                    DEFAULT_NAMESPACE, skillName, draftVersion, true);
            assertTrue(published, "Publish should succeed");
            Thread.sleep(500);

            // Verify: get version detail
            Skill versionDetail = aiService.skill().getSkillVersionDetail(
                    DEFAULT_NAMESPACE, skillName, draftVersion);
            assertNotNull(versionDetail, "Published skill version should be retrievable");
            assertEquals(skillName, versionDetail.getName(), "Skill name should match");
        } finally {
            try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
        }
    }

    // ==================== P1: Skill Listing and Search ====================

    /**
     * SKILL-005: List skills with pagination
     */
    @Test
    @Order(5)
    void testListSkills() throws Exception {
        String prefix = "skill-list-" + UUID.randomUUID().toString().substring(0, 6);
        List<String> names = new ArrayList<>();

        try {
            for (int i = 0; i < 3; i++) {
                String name = prefix + "-" + i;
                names.add(name);
                String card = buildSkillCardJson(name, "List test " + i);
                aiService.skill().createDraft(DEFAULT_NAMESPACE, card);
            }
            Thread.sleep(500);

            Page<SkillSummary> page = aiService.skill().listSkills(
                    DEFAULT_NAMESPACE, prefix, "blur", 1, 100);
            assertNotNull(page, "Skill list should not be null");
            assertNotNull(page.getPageItems(), "Page items should not be null");
            assertTrue(page.getPageItems().size() >= 3,
                    "Should have at least 3 skills, got: " + page.getPageItems().size());
        } finally {
            for (String name : names) {
                try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, name); } catch (Exception ignored) {}
            }
        }
    }

    /**
     * SKILL-006: Accurate search skills
     */
    @Test
    @Order(6)
    void testSearchSkillsAccurate() throws Exception {
        String skillName = "skill-accurate-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            String card = buildSkillCardJson(skillName, "Accurate search test");
            aiService.skill().createDraft(DEFAULT_NAMESPACE, card);
            Thread.sleep(500);

            Page<SkillSummary> page = aiService.skill().listSkills(
                    DEFAULT_NAMESPACE, skillName, "accurate", 1, 100);
            assertNotNull(page, "Search result should not be null");
            boolean found = page.getPageItems().stream()
                    .anyMatch(s -> skillName.equals(s.getName()));
            assertTrue(found, "Accurate search should find the exact skill");
        } finally {
            try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
        }
    }

    // ==================== P1: Skill Governance ====================

    /**
     * SKILL-007: Update biz tags
     */
    @Test
    @Order(7)
    void testUpdateBizTags() throws Exception {
        String skillName = "skill-tags-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            String card = buildSkillCardJson(skillName, "Tags test");
            aiService.skill().createDraft(DEFAULT_NAMESPACE, card);
            Thread.sleep(500);

            boolean updated = aiService.skill().updateBizTags(
                    DEFAULT_NAMESPACE, skillName, "[\"ai\",\"test\",\"batata\"]");
            assertTrue(updated, "Update biz tags should succeed");

            SkillMeta meta = aiService.skill().getSkillMeta(DEFAULT_NAMESPACE, skillName);
            assertNotNull(meta, "Skill meta should be retrievable");
            assertNotNull(meta.getBizTags(), "Biz tags should not be null");
            assertTrue(meta.getBizTags().contains("ai"), "Biz tags should contain 'ai'");
        } finally {
            try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
        }
    }

    /**
     * SKILL-008: Update scope (PUBLIC/PRIVATE)
     */
    @Test
    @Order(8)
    void testUpdateScope() throws Exception {
        String skillName = "skill-scope-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            String card = buildSkillCardJson(skillName, "Scope test");
            aiService.skill().createDraft(DEFAULT_NAMESPACE, card);
            Thread.sleep(500);

            boolean updated = aiService.skill().updateScope(DEFAULT_NAMESPACE, skillName, "PRIVATE");
            assertTrue(updated, "Update scope should succeed");

            SkillMeta meta = aiService.skill().getSkillMeta(DEFAULT_NAMESPACE, skillName);
            assertNotNull(meta);
            assertEquals("PRIVATE", meta.getScope(), "Scope should be PRIVATE");
        } finally {
            try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
        }
    }

    /**
     * SKILL-009: Delete skill entirely
     */
    @Test
    @Order(9)
    void testDeleteSkill() throws Exception {
        String skillName = "skill-delete-" + UUID.randomUUID().toString().substring(0, 8);

        String card = buildSkillCardJson(skillName, "To be deleted");
        aiService.skill().createDraft(DEFAULT_NAMESPACE, card);
        Thread.sleep(500);

        boolean deleted = aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName);
        assertTrue(deleted, "Skill deletion should succeed");

        try {
            SkillMeta meta = aiService.skill().getSkillMeta(DEFAULT_NAMESPACE, skillName);
            assertNull(meta, "Deleted skill should not be retrievable");
        } catch (NacosException e) {
            assertTrue(true, "Deleted skill correctly throws exception");
        }
    }

    /**
     * SKILL-010: Online/offline status management
     */
    @Test
    @Order(10)
    void testOnlineOfflineStatus() throws Exception {
        String skillName = "skill-status-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Create, submit, publish
            String card = buildSkillCardJson(skillName, "Status test");
            String version = aiService.skill().createDraft(DEFAULT_NAMESPACE, card);
            Thread.sleep(300);
            aiService.skill().submit(DEFAULT_NAMESPACE, skillName, version);
            Thread.sleep(300);
            aiService.skill().publish(DEFAULT_NAMESPACE, skillName, version, true);
            Thread.sleep(500);

            // Set offline
            boolean offlined = aiService.skill().changeOnlineStatus(
                    DEFAULT_NAMESPACE, skillName, "PUBLIC", version, false);
            assertTrue(offlined, "Offline should succeed");

            // Set back online
            boolean onlined = aiService.skill().changeOnlineStatus(
                    DEFAULT_NAMESPACE, skillName, "PUBLIC", version, true);
            assertTrue(onlined, "Online should succeed");
        } finally {
            try { aiService.skill().deleteSkill(DEFAULT_NAMESPACE, skillName); } catch (Exception ignored) {}
        }
    }
}

package io.batata.tests;

import com.alibaba.nacos.api.ai.model.a2a.AgentCard;
import com.alibaba.nacos.api.ai.model.a2a.AgentCardDetailInfo;
import com.alibaba.nacos.api.ai.model.a2a.AgentCardVersionInfo;
import com.alibaba.nacos.api.ai.model.a2a.AgentCapabilities;
import com.alibaba.nacos.api.ai.model.a2a.AgentProvider;
import com.alibaba.nacos.api.ai.model.a2a.AgentSkill;
import com.alibaba.nacos.api.ai.model.a2a.AgentVersionDetail;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerFactory;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerService;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos AI A2A (Agent-to-Agent) Management Tests
 *
 * Tests for agent registration, discovery, versioning, and lifecycle
 * via AiMaintainerService.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAiA2aTest {

    private static AiMaintainerService aiService;

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
        System.out.println("AI A2A Test Setup - Server: " + serverAddr);
    }

    private AgentCard buildAgentCard(String name, String version) {
        AgentCard card = new AgentCard();
        card.setName(name);
        card.setVersion(version);
        card.setDescription("Test agent: " + name);

        card.setUrl("http://localhost:9000/a2a/" + name);

        AgentCapabilities caps = new AgentCapabilities();
        caps.setStreaming(true);
        card.setCapabilities(caps);

        AgentProvider provider = new AgentProvider();
        provider.setOrganization("batata-test");
        card.setProvider(provider);

        List<AgentSkill> skills = new ArrayList<>();
        AgentSkill skill = new AgentSkill();
        skill.setId("skill-" + name);
        skill.setName("Test Skill");
        skill.setDescription("A test skill for " + name);
        skills.add(skill);
        card.setSkills(skills);

        return card;
    }

    // ==================== P0: Agent CRUD ====================

    /**
     * A2A-001: Register and get agent
     */
    @Test
    @Order(1)
    void testRegisterAndGetAgent() throws Exception {
        String agentName = "a2a-test-" + UUID.randomUUID().toString().substring(0, 8);
        AgentCard card = buildAgentCard(agentName, "1.0.0");

        try {
            boolean registered = aiService.a2a().registerAgent(card);
            assertTrue(registered, "Agent registration should succeed");

            AgentCardDetailInfo detail = aiService.a2a().getAgentCard(agentName);
            assertNotNull(detail, "Agent card should be retrievable");
            assertEquals(agentName, detail.getName(), "Agent name should match");
        } finally {
            try { aiService.a2a().deleteAgent(agentName); } catch (Exception ignored) {}
        }
    }

    /**
     * A2A-002: List agents with pagination
     */
    @Test
    @Order(2)
    void testListAgents() throws Exception {
        String prefix = "a2a-list-" + UUID.randomUUID().toString().substring(0, 6);
        List<String> names = new ArrayList<>();

        try {
            for (int i = 0; i < 3; i++) {
                String name = prefix + "-" + i;
                names.add(name);
                aiService.a2a().registerAgent(buildAgentCard(name, "1.0.0"));
            }
            Thread.sleep(500);

            Page<AgentCardVersionInfo> page = aiService.a2a().listAgentCards(1, 100);
            assertNotNull(page, "Agent list should not be null");
            assertTrue(page.getPageItems().size() >= 3,
                    "Should have at least 3 agents, got: " + page.getPageItems().size());
        } finally {
            for (String name : names) {
                try { aiService.a2a().deleteAgent(name); } catch (Exception ignored) {}
            }
        }
    }

    /**
     * A2A-003: Search agents by name
     */
    @Test
    @Order(3)
    void testSearchAgents() throws Exception {
        String prefix = "a2a-search-" + UUID.randomUUID().toString().substring(0, 6);
        String name1 = prefix + "-alpha";
        String name2 = prefix + "-beta";

        try {
            aiService.a2a().registerAgent(buildAgentCard(name1, "1.0.0"));
            aiService.a2a().registerAgent(buildAgentCard(name2, "1.0.0"));
            Thread.sleep(500);

            Page<AgentCardVersionInfo> result = aiService.a2a().searchAgentCardsByName(prefix, 1, 100);
            assertNotNull(result, "Search result should not be null");
            assertTrue(result.getPageItems().size() >= 2,
                    "Should find at least 2 matching agents");
        } finally {
            try { aiService.a2a().deleteAgent(name1); } catch (Exception ignored) {}
            try { aiService.a2a().deleteAgent(name2); } catch (Exception ignored) {}
        }
    }

    /**
     * A2A-004: Update agent card
     */
    @Test
    @Order(4)
    void testUpdateAgentCard() throws Exception {
        String agentName = "a2a-update-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            aiService.a2a().registerAgent(buildAgentCard(agentName, "1.0.0"));
            Thread.sleep(500);

            // Update with new version
            AgentCard updatedCard = buildAgentCard(agentName, "2.0.0");
            updatedCard.setDescription("Updated agent description");
            boolean updated = aiService.a2a().updateAgentCard(updatedCard, "", true);
            assertTrue(updated, "Agent update should succeed");

            // Verify update
            AgentCardDetailInfo detail = aiService.a2a().getAgentCard(agentName);
            assertNotNull(detail, "Updated agent should be retrievable");
            assertEquals("Updated agent description", detail.getDescription(),
                    "Description should be updated");
        } finally {
            try { aiService.a2a().deleteAgent(agentName); } catch (Exception ignored) {}
        }
    }

    /**
     * A2A-005: Delete agent
     */
    @Test
    @Order(5)
    void testDeleteAgent() throws Exception {
        String agentName = "a2a-delete-" + UUID.randomUUID().toString().substring(0, 8);
        aiService.a2a().registerAgent(buildAgentCard(agentName, "1.0.0"));
        Thread.sleep(500);

        boolean deleted = aiService.a2a().deleteAgent(agentName);
        assertTrue(deleted, "Agent deletion should succeed");

        try {
            AgentCardDetailInfo detail = aiService.a2a().getAgentCard(agentName);
            assertNull(detail, "Deleted agent should not be retrievable");
        } catch (NacosException e) {
            assertTrue(true, "Deleted agent correctly throws exception");
        }
    }

    /**
     * A2A-006: Agent version management
     */
    @Test
    @Order(6)
    void testAgentVersionManagement() throws Exception {
        String agentName = "a2a-version-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            // Register v1
            aiService.a2a().registerAgent(buildAgentCard(agentName, "1.0.0"));
            Thread.sleep(500);

            // Update to v2
            AgentCard v2 = buildAgentCard(agentName, "2.0.0");
            v2.setDescription("Version 2");
            aiService.a2a().updateAgentCard(v2, "", true);
            Thread.sleep(500);

            // List all versions
            List<AgentVersionDetail> versions = aiService.a2a().listAllVersionOfAgent(agentName);
            assertNotNull(versions, "Version list should not be null");
            assertTrue(versions.size() >= 2,
                    "Should have at least 2 versions, got: " + versions.size());
        } finally {
            try { aiService.a2a().deleteAgent(agentName); } catch (Exception ignored) {}
        }
    }

    /**
     * A2A-007: Get non-existent agent should fail
     */
    @Test
    @Order(7)
    void testGetNonExistentAgent() {
        String agentName = "a2a-nonexist-" + UUID.randomUUID().toString().substring(0, 8);
        try {
            AgentCardDetailInfo detail = aiService.a2a().getAgentCard(agentName);
            assertNull(detail, "Non-existent agent should return null");
        } catch (NacosException e) {
            assertTrue(true, "Non-existent agent correctly throws exception");
        }
    }
}

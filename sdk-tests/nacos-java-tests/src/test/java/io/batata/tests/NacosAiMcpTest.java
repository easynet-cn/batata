package io.batata.tests;

import com.alibaba.nacos.api.ai.model.mcp.McpEndpointSpec;
import com.alibaba.nacos.api.ai.model.mcp.McpServerBasicInfo;
import com.alibaba.nacos.api.ai.model.mcp.McpServerDetailInfo;
import com.alibaba.nacos.api.exception.NacosException;
import com.alibaba.nacos.api.model.Page;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerFactory;
import com.alibaba.nacos.maintainer.client.ai.AiMaintainerService;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.Disabled;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Nacos AI MCP Server Management Tests
 *
 * Tests for MCP (Model Context Protocol) server CRUD operations via AiMaintainerService.
 * Covers: create remote MCP servers, get detail, list, search, update, delete.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class NacosAiMcpTest {

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
        System.out.println("AI MCP Test Setup - Server: " + serverAddr);
    }

    private static McpEndpointSpec buildDirectEndpoint(String address, int port) {
        McpEndpointSpec spec = new McpEndpointSpec();
        spec.setType("DIRECT");
        Map<String, String> data = new HashMap<>();
        data.put("address", address);
        data.put("port", String.valueOf(port));
        spec.setData(data);
        return spec;
    }

    private static McpServerBasicInfo buildServerSpec(String name, String version, String description) {
        McpServerBasicInfo spec = new McpServerBasicInfo();
        spec.setName(name);
        spec.setVersion(version);
        spec.setDescription(description);
        spec.setProtocol("sse");
        return spec;
    }

    // ==================== P0: MCP Server CRUD ====================

    /**
     * MCP-001: Create and get remote MCP server
     */
    @Test
    @Order(1)
    void testCreateAndGetRemoteMcpServer() throws Exception {
        String mcpName = "mcp-test-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            String result = aiService.mcp().createRemoteMcpServer(
                    mcpName, buildServerSpec(mcpName, "1.0.0", "Test MCP server"),
                    null, buildDirectEndpoint("localhost", 3000));
            assertNotNull(result, "Create MCP server should return a result");

            McpServerDetailInfo detail = aiService.mcp().getMcpServerDetail(mcpName);
            assertNotNull(detail, "MCP server detail should not be null");
            assertEquals(mcpName, detail.getName(), "MCP server name should match");
            assertEquals("1.0.0", detail.getVersion(), "Version should match");
            assertEquals("Test MCP server", detail.getDescription(), "Description should match");
        } finally {
            try { aiService.mcp().deleteMcpServer(mcpName); } catch (Exception ignored) {}
        }
    }

    /**
     * MCP-002: List MCP servers with pagination
     */
    @Test
    @Order(2)
    void testListMcpServers() throws Exception {
        String prefix = "mcp-list-" + UUID.randomUUID().toString().substring(0, 6);
        List<String> names = new ArrayList<>();

        try {
            for (int i = 0; i < 3; i++) {
                String name = prefix + "-" + i;
                names.add(name);
                aiService.mcp().createRemoteMcpServer(name,
                        buildServerSpec(name, "1.0.0", "List test " + i),
                        null, buildDirectEndpoint("localhost", 3000 + i));
            }
            Thread.sleep(500);

            Page<McpServerBasicInfo> page = aiService.mcp().listMcpServer(1, 100);
            assertNotNull(page, "MCP server list should not be null");
            assertNotNull(page.getPageItems(), "Page items should not be null");
            assertTrue(page.getPageItems().size() >= 3,
                    "Should have at least 3 MCP servers, got: " + page.getPageItems().size());
        } finally {
            for (String name : names) {
                try { aiService.mcp().deleteMcpServer(name); } catch (Exception ignored) {}
            }
        }
    }

    /**
     * MCP-003: Search MCP servers by name pattern
     */
    @Test
    @Order(3)
    void testSearchMcpServers() throws Exception {
        String prefix = "mcp-search-" + UUID.randomUUID().toString().substring(0, 6);
        String name1 = prefix + "-alpha";
        String name2 = prefix + "-beta";

        try {
            aiService.mcp().createRemoteMcpServer(name1,
                    buildServerSpec(name1, "1.0.0", "Search alpha"),
                    null, buildDirectEndpoint("localhost", 3000));
            aiService.mcp().createRemoteMcpServer(name2,
                    buildServerSpec(name2, "1.0.0", "Search beta"),
                    null, buildDirectEndpoint("localhost", 3001));
            Thread.sleep(500);

            Page<McpServerBasicInfo> result = aiService.mcp().searchMcpServer(prefix, 1, 100);
            assertNotNull(result, "Search result should not be null");
            assertTrue(result.getPageItems().size() >= 2,
                    "Should find at least 2 matching servers");
        } finally {
            try { aiService.mcp().deleteMcpServer(name1); } catch (Exception ignored) {}
            try { aiService.mcp().deleteMcpServer(name2); } catch (Exception ignored) {}
        }
    }

    /**
     * MCP-004: Update MCP server
     */
    @Test
    @Order(4)
    void testUpdateMcpServer() throws Exception {
        String mcpName = "mcp-update-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            aiService.mcp().createRemoteMcpServer(mcpName,
                    buildServerSpec(mcpName, "1.0.0", "Original description"),
                    null, buildDirectEndpoint("localhost", 3000));
            Thread.sleep(500);

            boolean updated = aiService.mcp().updateMcpServer(mcpName, true,
                    buildServerSpec(mcpName, "2.0.0", "Updated description"),
                    null, buildDirectEndpoint("localhost", 3001));
            assertTrue(updated, "MCP server update should succeed");

            McpServerDetailInfo detail = aiService.mcp().getMcpServerDetail(mcpName);
            assertNotNull(detail, "Updated MCP server should be retrievable");
            assertEquals("Updated description", detail.getDescription(),
                    "Description should be updated");
        } finally {
            try { aiService.mcp().deleteMcpServer(mcpName); } catch (Exception ignored) {}
        }
    }

    /**
     * MCP-005: Delete MCP server
     */
    @Test
    @Order(5)
    void testDeleteMcpServer() throws Exception {
        String mcpName = "mcp-delete-" + UUID.randomUUID().toString().substring(0, 8);

        aiService.mcp().createRemoteMcpServer(mcpName,
                buildServerSpec(mcpName, "1.0.0", "To be deleted"),
                null, buildDirectEndpoint("localhost", 3000));
        Thread.sleep(500);

        boolean deleted = aiService.mcp().deleteMcpServer(mcpName);
        assertTrue(deleted, "MCP server deletion should succeed");

        try {
            McpServerDetailInfo detail = aiService.mcp().getMcpServerDetail(mcpName);
            assertNull(detail, "Deleted MCP server should not be retrievable");
        } catch (NacosException e) {
            assertTrue(true, "MCP server correctly not found after deletion");
        }
    }

    /**
     * MCP-006: Get non-existent MCP server should fail
     */
    @Test
    @Order(6)
    void testGetNonExistentMcpServer() {
        String mcpName = "mcp-nonexist-" + UUID.randomUUID().toString().substring(0, 8);
        try {
            McpServerDetailInfo detail = aiService.mcp().getMcpServerDetail(mcpName);
            assertNull(detail, "Non-existent MCP server should return null");
        } catch (NacosException e) {
            assertTrue(true, "Non-existent MCP server correctly throws exception");
        }
    }

    /**
     * MCP-007: Create MCP server with version and verify version in detail
     */
    @Test
    @Order(7)
    void testMcpServerVersioning() throws Exception {
        String mcpName = "mcp-version-" + UUID.randomUUID().toString().substring(0, 8);

        try {
            aiService.mcp().createRemoteMcpServer(mcpName,
                    buildServerSpec(mcpName, "1.0.0", "Version test"),
                    null, buildDirectEndpoint("localhost", 3000));
            Thread.sleep(500);

            // Get by specific version
            McpServerDetailInfo detail = aiService.mcp().getMcpServerDetail(mcpName, "1.0.0");
            assertNotNull(detail, "Should get MCP server by version");
            assertEquals("1.0.0", detail.getVersion(), "Version should match");
        } finally {
            try { aiService.mcp().deleteMcpServer(mcpName); } catch (Exception ignored) {}
        }
    }
}

package io.batata.tests;

import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;
import com.ctrip.framework.apollo.openapi.dto.OpenItemDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenPageDTO;
import org.junit.jupiter.api.*;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class ApolloConfigItemTest extends ApolloTestBase {

    private String testAppId;
    private String testNamespaceName;

    @BeforeAll
    void setup() {
        String openApiUrl = System.getProperty("apollo.openapi.url", "http://127.0.0.1:8080");
        String token = System.getProperty("apollo.openapi.token", "admin");

        openApiClient = ApolloOpenApiClient.newBuilder()
                .withPortalUrl(openApiUrl)
                .withToken(token)
                .build();
        assertNotNull(openApiClient);
    }

    @BeforeEach
    void beforeEach() {
        testAppId = createTestApp();
        testNamespaceName = createTestNamespace(testAppId);
    }

    @Test
    @Order(1)
    void testCreateItem() {
        createConfigItem(testAppId, testNamespaceName, "test.key", "test.value");

        OpenItemDTO item = openApiClient.getItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, "test.key");
        assertNotNull(item, "Item should exist after creation");
        assertEquals("test.key", item.getKey());
        assertEquals("test.value", item.getValue());
    }

    @Test
    @Order(2)
    void testGetItem() {
        createConfigItem(testAppId, testNamespaceName, "test.key", "test.value");

        OpenItemDTO item = openApiClient.getItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, "test.key");
        assertNotNull(item);
        assertEquals("test.key", item.getKey());
        assertEquals("test.value", item.getValue());
    }

    @Test
    @Order(3)
    void testGetItemNotFound() {
        OpenItemDTO item = openApiClient.getItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, "non.existent.key");
        assertNull(item, "Item should be null when not found (Apollo SDK returns null on 404)");
    }

    @Test
    @Order(4)
    void testUpdateItem() {
        createConfigItem(testAppId, testNamespaceName, "test.key", "original");
        updateConfigItem(testAppId, testNamespaceName, "test.key", "updated");

        OpenItemDTO item = openApiClient.getItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, "test.key");
        assertNotNull(item);
        assertEquals("updated", item.getValue());
    }

    @Test
    @Order(5)
    void testRemoveItem() {
        createConfigItem(testAppId, testNamespaceName, "test.key", "test.value");

        openApiClient.removeItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, "test.key", OPERATOR);

        OpenItemDTO item = openApiClient.getItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, "test.key");
        assertNull(item, "Item should be null after removal (Apollo SDK returns null on 404)");
    }

    @Test
    @Order(6)
    void testCreateDuplicateItem() {
        createConfigItem(testAppId, testNamespaceName, "test.key", "first");

        assertThrows(Exception.class, () -> {
            createConfigItem(testAppId, testNamespaceName, "test.key", "second");
        });
    }

    @Test
    @Order(7)
    void testFindItemsByNamespace() {
        createConfigItem(testAppId, testNamespaceName, "key1", "value1");
        createConfigItem(testAppId, testNamespaceName, "key2", "value2");
        createConfigItem(testAppId, testNamespaceName, "key3", "value3");

        OpenPageDTO<OpenItemDTO> page = openApiClient.findItemsByNamespace(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, 0, 10);
        assertNotNull(page, "Page should not be null");
        assertTrue(page.hasContent(), "Page should have content");
        assertEquals(3, page.getTotal(), "Total items should be 3");

        List<OpenItemDTO> items = page.getContent();
        assertNotNull(items);
        assertEquals(3, items.size(), "Page content size should be 3");
    }

    @Test
    @Order(8)
    void testCreateOrUpdateItem() {
        createConfigItem(testAppId, testNamespaceName, "upsert.key", "initial");

        OpenItemDTO updateItem = new OpenItemDTO();
        updateItem.setKey("upsert.key");
        updateItem.setValue("upserted");
        updateItem.setComment("Upserted item");
        updateItem.setDataChangeCreatedBy(OPERATOR);
        updateItem.setDataChangeLastModifiedBy(OPERATOR);
        openApiClient.createOrUpdateItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, updateItem);

        OpenItemDTO item = openApiClient.getItem(
                testAppId, DEFAULT_ENV, DEFAULT_CLUSTER, testNamespaceName, "upsert.key");
        assertNotNull(item);
        assertEquals("upserted", item.getValue());
    }
}

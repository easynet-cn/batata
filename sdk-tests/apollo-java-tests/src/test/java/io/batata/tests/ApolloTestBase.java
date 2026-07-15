package io.batata.tests;

import com.ctrip.framework.apollo.openapi.client.ApolloOpenApiClient;
import com.ctrip.framework.apollo.openapi.dto.NamespaceReleaseDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenAppDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenAppNamespaceDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenCreateAppDTO;
import com.ctrip.framework.apollo.openapi.dto.OpenItemDTO;

import java.util.Collections;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

public class ApolloTestBase {

    protected static ApolloOpenApiClient openApiClient;
    protected static final String DEFAULT_CLUSTER = "default";
    protected static final String DEFAULT_NAMESPACE = "application";
    protected static final String DEFAULT_ENV = "DEV";
    protected static final String OPERATOR = "test";

    protected static String createTestApp() {
        String appId = "test-app-" + UUID.randomUUID().toString().substring(0, 8);

        OpenAppDTO app = new OpenAppDTO();
        app.setAppId(appId);
        app.setName("Test App " + appId);
        app.setOrgId("TEST_ORG");
        app.setOrgName("Test Organization");
        app.setOwnerName("Test Owner");
        app.setOwnerEmail("test@example.com");
        app.setDataChangeCreatedBy(OPERATOR);

        OpenCreateAppDTO createAppDTO = new OpenCreateAppDTO();
        createAppDTO.setApp(app);
        createAppDTO.setAdmins(Collections.singleton(OPERATOR));
        createAppDTO.setAssignAppRoleToSelf(true);
        openApiClient.createApp(createAppDTO);
        return appId;
    }

    protected static String createTestNamespace(String appId) {
        String namespaceName = "test-ns-" + UUID.randomUUID().toString().substring(0, 8);

        OpenAppNamespaceDTO appNamespace = new OpenAppNamespaceDTO();
        appNamespace.setAppId(appId);
        appNamespace.setName(namespaceName);
        appNamespace.setFormat("properties");
        appNamespace.setPublic(false);
        appNamespace.setDataChangeCreatedBy(OPERATOR);
        openApiClient.createAppNamespace(appNamespace);
        return namespaceName;
    }

    protected static void createConfigItem(String appId, String namespaceName, String key, String value) {
        OpenItemDTO item = new OpenItemDTO();
        item.setKey(key);
        item.setValue(value);
        item.setComment("Test item");
        item.setDataChangeCreatedBy(OPERATOR);
        openApiClient.createItem(appId, DEFAULT_ENV, DEFAULT_CLUSTER, namespaceName, item);
    }

    protected static void updateConfigItem(String appId, String namespaceName, String key, String value) {
        OpenItemDTO item = new OpenItemDTO();
        item.setKey(key);
        item.setValue(value);
        item.setComment("Updated test item");
        item.setDataChangeLastModifiedBy(OPERATOR);
        openApiClient.updateItem(appId, DEFAULT_ENV, DEFAULT_CLUSTER, namespaceName, item);
    }

    protected static void deleteConfigItem(String appId, String namespaceName, String key) {
        try {
            openApiClient.removeItem(appId, DEFAULT_ENV, DEFAULT_CLUSTER, namespaceName, key, OPERATOR);
        } catch (Exception ignored) {
        }
    }

    protected static void releaseNamespace(String appId, String namespaceName) {
        NamespaceReleaseDTO release = new NamespaceReleaseDTO();
        release.setReleaseTitle("Test release " + System.currentTimeMillis());
        release.setReleaseComment("Test release comment");
        release.setReleasedBy(OPERATOR);
        release.setEmergencyPublish(false);
        openApiClient.publishNamespace(appId, DEFAULT_ENV, DEFAULT_CLUSTER, namespaceName, release);
    }

    protected static Map<String, String> createTestItems(String appId, String namespaceName, int count) {
        Map<String, String> items = new HashMap<>();
        for (int i = 0; i < count; i++) {
            String key = "test.key." + i;
            String value = "test.value." + i;
            createConfigItem(appId, namespaceName, key, value);
            items.put(key, value);
        }
        return items;
    }
}

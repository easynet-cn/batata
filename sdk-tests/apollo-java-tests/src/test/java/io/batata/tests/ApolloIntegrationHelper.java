package io.batata.tests;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.HashMap;
import java.util.Map;

/**
 * HTTP helpers for exercising batata's Apollo-compatible endpoints that the
 * official Apollo OpenAPI client does not cover (AccessKey management, gray
 * release setup). Also provides the HMAC-SHA1 signature used by A3.
 */
public final class ApolloIntegrationHelper {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    private ApolloIntegrationHelper() {
    }

    public static String meta() {
        return System.getProperty("apollo.meta", "http://127.0.0.1:8080");
    }

    public static String token() {
        return System.getProperty("apollo.openapi.token", "admin");
    }

    public static class HttpResult {
        public final int status;
        public final String body;

        HttpResult(int status, String body) {
            this.status = status;
            this.body = body;
        }
    }

    public static HttpResult request(String method, String path, String body, Map<String, String> headers) {
        try {
            HttpClient client = HttpClient.newHttpClient();
            HttpRequest.Builder b = HttpRequest.newBuilder()
                    .uri(URI.create(meta() + path));
            if (headers != null) {
                headers.forEach(b::header);
            }
            HttpRequest.BodyPublisher publisher = body != null
                    ? HttpRequest.BodyPublishers.ofString(body, StandardCharsets.UTF_8)
                    : HttpRequest.BodyPublishers.noBody();
            b.method(method, publisher);
            HttpResponse<String> resp = client.send(b.build(), HttpResponse.BodyHandlers.ofString());
            return new HttpResult(resp.statusCode(), resp.body());
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    /** Create an access key for the given app and return its secret. */
    public static String createAccessKey(String appId) {
        Map<String, String> h = new HashMap<>();
        h.put("Authorization", token());
        h.put("Content-Type", "application/json");
        HttpResult r = request("POST", "/openapi/v1/apps/" + appId + "/accesskeys",
                "{\"createdBy\":\"test\"}", h);
        if (r.status >= 400) {
            throw new IllegalStateException("createAccessKey failed: " + r.status + " " + r.body);
        }
        try {
            JsonNode node = MAPPER.readTree(r.body);
            return node.get("secret").asText();
        } catch (Exception e) {
            throw new RuntimeException("failed to parse access key response: " + r.body, e);
        }
    }

    /** Upstream createBranch: returns the generated branch cluster name. */
    public static String createBranch(String appId, String ns) {
        Map<String, String> h = new HashMap<>();
        h.put("Authorization", token());
        h.put("Content-Type", "application/json");
        HttpResult r = request("POST",
                "/openapi/v1/envs/DEV/apps/" + appId + "/clusters/default/namespaces/" + ns + "/branches",
                "{\"operator\":\"test\"}", h);
        if (r.status >= 400) {
            throw new IllegalStateException("createBranch failed: " + r.status + " " + r.body);
        }
        try {
            return MAPPER.readTree(r.body).get("clusterName").asText();
        } catch (Exception e) {
            throw new RuntimeException("bad branch response: " + r.body, e);
        }
    }

    /** Create an item on an ARBITRARY cluster (used for branch edits). */
    public static void createItemOnCluster(String appId, String cluster, String ns, String key, String value) {
        Map<String, String> h = new HashMap<>();
        h.put("Authorization", token());
        h.put("Content-Type", "application/json");
        String body;
        try {
            com.fasterxml.jackson.databind.node.ObjectNode n = MAPPER.createObjectNode();
            n.put("key", key);
            n.put("value", value);
            n.put("comment", "branch edit");
            n.put("dataChangeCreatedBy", "test");
            body = MAPPER.writeValueAsString(n);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        HttpResult r = request("POST",
                "/openapi/v1/envs/DEV/apps/" + appId + "/clusters/" + cluster + "/namespaces/" + ns + "/items",
                body, h);
        if (r.status >= 400) {
            throw new IllegalStateException("createItemOnCluster failed: " + r.status + " " + r.body);
        }
    }

    /** Build a gray-rule request body; upstream stores rules as an escaped JSON string column. */
    private static String grayRuleBody(String appId, String ns, String branch, String rulesJson, int releaseId) {
        try {
            com.fasterxml.jackson.databind.node.ObjectNode n = MAPPER.createObjectNode();
            n.put("appId", appId);
            n.put("clusterName", "default");
            n.put("namespaceName", ns);
            n.put("branchName", branch);
            n.put("rules", rulesJson); // String field -> escaped JSON
            n.put("releaseId", releaseId);
            n.put("branchStatus", 1);
            n.put("dataChangeCreatedBy", "test");
            return MAPPER.writeValueAsString(n);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    /** Create a gray release rule (branch) for a namespace. */
    public static void createGrayRule(String appId, String ns, String branch, String rulesJson, int releaseId) {
        Map<String, String> h = new HashMap<>();
        h.put("Authorization", token());
        h.put("Content-Type", "application/json");
        HttpResult r = request("POST",
                "/apps/" + appId + "/clusters/default/namespaces/" + ns + "/gray-release-rules",
                grayRuleBody(appId, ns, branch, rulesJson, releaseId), h);
        if (r.status >= 400) {
            throw new IllegalStateException("createGrayRule failed: " + r.status + " " + r.body);
        }
    }

    /** Update an existing gray release rule with the resolved gray release id. */
    public static void updateGrayRule(String appId, String ns, String branch, String rulesJson, int releaseId) {
        Map<String, String> h = new HashMap<>();
        h.put("Authorization", token());
        h.put("Content-Type", "application/json");
        HttpResult r = request("PUT",
                "/apps/" + appId + "/clusters/default/namespaces/" + ns + "/gray-release-rules/" + branch,
                grayRuleBody(appId, ns, branch, rulesJson, releaseId), h);
        if (r.status >= 400) {
            throw new IllegalStateException("updateGrayRule failed: " + r.status + " " + r.body);
        }
    }

    /** Publish a gray release (captures the parent namespace's current items). */
    public static int createGrayRelease(String appId, String ns, String branch) {
        Map<String, String> h = new HashMap<>();
        h.put("Authorization", token());
        h.put("Content-Type", "application/json");
        String body = "{\"releaseTitle\":\"gray-release\",\"releaseComment\":\"gray\",\"releasedBy\":\"test\"}";
        HttpResult r = request("POST",
                "/openapi/v1/envs/DEV/apps/" + appId + "/clusters/default/namespaces/"
                        + ns + "/branches/" + branch + "/releases", body, h);
        if (r.status >= 400) {
            throw new IllegalStateException("createGrayRelease failed: " + r.status + " " + r.body);
        }
        try {
            JsonNode node = MAPPER.readTree(r.body);
            return node.get("id").asInt();
        } catch (Exception e) {
            throw new RuntimeException("failed to parse gray release response: " + r.body, e);
        }
    }

    /** Apollo access-key signature: Base64(HMAC-SHA1(secret, timestamp + "\n" + pathWithQuery)). */
    public static String sign(String secret, long timestamp, String pathWithQuery) throws Exception {
        String data = timestamp + "\n" + pathWithQuery;
        Mac mac = Mac.getInstance("HmacSHA1");
        mac.init(new SecretKeySpec(secret.getBytes(StandardCharsets.UTF_8), "HmacSHA1"));
        byte[] raw = mac.doFinal(data.getBytes(StandardCharsets.UTF_8));
        return Base64.getEncoder().encodeToString(raw);
    }
}

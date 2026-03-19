package io.batata.tests;

import com.alibaba.nacos.api.grpc.auto.BiRequestStreamGrpc;
import com.alibaba.nacos.api.grpc.auto.Payload;
import com.alibaba.nacos.api.grpc.auto.RequestGrpc;
import com.alibaba.nacos.common.remote.client.grpc.GrpcUtils;
import com.alibaba.nacos.api.remote.request.Request;
import com.alibaba.nacos.api.remote.request.ConnectionSetupRequest;
import com.alibaba.nacos.api.remote.request.ServerCheckRequest;
import com.alibaba.nacos.api.remote.response.ServerCheckResponse;
import com.alibaba.nacos.shaded.io.grpc.ManagedChannel;
import com.alibaba.nacos.shaded.io.grpc.ManagedChannelBuilder;
import com.alibaba.nacos.shaded.io.grpc.stub.StreamObserver;
import org.junit.jupiter.api.*;

import java.util.*;
import java.util.concurrent.*;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Diagnostic test to capture raw gRPC payloads from Batata server
 * and verify they can be parsed by Nacos SDK.
 */
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class GrpcPayloadDiagnosticTest {

    private static ManagedChannel channel;
    private static final int GRPC_PORT = 9848;

    @BeforeAll
    static void setup() {
        String host = System.getProperty("nacos.server", "127.0.0.1:8848").split(":")[0];
        channel = ManagedChannelBuilder.forAddress(host, GRPC_PORT)
                .usePlaintext()
                .build();
    }

    @AfterAll
    static void teardown() {
        if (channel != null) {
            channel.shutdown();
        }
    }

    /**
     * DIAG-001: Verify ServerCheckResponse from Batata is parseable by SDK
     */
    @Test
    @Order(1)
    void testServerCheckResponse() throws Exception {
        RequestGrpc.RequestBlockingStub stub = RequestGrpc.newBlockingStub(channel);

        ServerCheckRequest request = new ServerCheckRequest();
        Payload requestPayload = GrpcUtils.convert(request);
        System.out.println("=== ServerCheckRequest ===");
        System.out.println("  Type: " + requestPayload.getMetadata().getType());

        Payload responsePayload = stub.request(requestPayload);
        System.out.println("=== ServerCheckResponse ===");
        System.out.println("  Type: " + responsePayload.getMetadata().getType());
        System.out.println("  Body: " + responsePayload.getBody().getValue().toStringUtf8());

        Object parsed = GrpcUtils.parse(responsePayload);
        System.out.println("  Parsed: " + parsed.getClass().getSimpleName());
        assertInstanceOf(ServerCheckResponse.class, parsed);

        ServerCheckResponse resp = (ServerCheckResponse) parsed;
        System.out.println("  ConnectionId: " + resp.getConnectionId());
        assertNotNull(resp.getConnectionId());
    }

    /**
     * DIAG-002: Verify SetupAckRequest from Batata is parseable by SDK
     * This is the key handshake that enables FuzzyWatch capability.
     */
    @Test
    @Order(2)
    void testSetupAckResponse() throws Exception {
        BiRequestStreamGrpc.BiRequestStreamStub biStub = BiRequestStreamGrpc.newStub(channel);

        CountDownLatch ackLatch = new CountDownLatch(1);
        List<String> receivedTypes = new CopyOnWriteArrayList<>();
        List<String> receivedBodies = new CopyOnWriteArrayList<>();
        List<String> parseErrors = new CopyOnWriteArrayList<>();

        StreamObserver<Payload> responseObserver = new StreamObserver<Payload>() {
            @Override
            public void onNext(Payload payload) {
                String type = payload.getMetadata().getType();
                String body = payload.getBody().getValue().toStringUtf8();
                System.out.println("=== Server Push ===");
                System.out.println("  Type: " + type);
                System.out.println("  Body: " + body);

                receivedTypes.add(type);
                receivedBodies.add(body);

                try {
                    Object parsed = GrpcUtils.parse(payload);
                    System.out.println("  Parsed: " + (parsed != null ? parsed.getClass().getName() : "null"));
                    if (parsed instanceof Request) {
                        Request req = (Request) parsed;
                        System.out.println("  RequestId: " + req.getRequestId());
                        System.out.println("  Module: " + req.getModule());
                    }
                } catch (Exception e) {
                    String err = type + ": " + e.getMessage();
                    System.out.println("  PARSE ERROR: " + err);
                    parseErrors.add(err);
                }

                if ("SetupAckRequest".equals(type)) {
                    ackLatch.countDown();
                }
            }

            @Override
            public void onError(Throwable t) {
                System.out.println("Stream error: " + t.getMessage());
            }

            @Override
            public void onCompleted() {
                System.out.println("Stream completed");
            }
        };

        StreamObserver<Payload> requestObserver = biStub.requestBiStream(responseObserver);

        // Build ConnectionSetupRequest with ability table
        ConnectionSetupRequest setup = new ConnectionSetupRequest();
        setup.setClientVersion("Nacos-Java-Client:v3.1.1");
        setup.setTenant("");
        setup.setLabels(Map.of("source", "diagnostic-test"));
        setup.setAbilityTable(Map.of("fuzzyWatch", true, "lock", true));

        Payload setupPayload = GrpcUtils.convert(setup);
        System.out.println("=== Sending ConnectionSetupRequest ===");
        System.out.println("  Type: " + setupPayload.getMetadata().getType());
        System.out.println("  Body: " + setupPayload.getBody().getValue().toStringUtf8());

        requestObserver.onNext(setupPayload);

        boolean received = ackLatch.await(10, TimeUnit.SECONDS);

        System.out.println("=== Summary ===");
        System.out.println("  SetupAck received: " + received);
        System.out.println("  Messages: " + receivedTypes);
        System.out.println("  Parse errors: " + parseErrors);

        assertTrue(received, "Should receive SetupAckRequest. Got: " + receivedTypes);
        assertTrue(parseErrors.isEmpty(), "All payloads should be parseable: " + parseErrors);

        requestObserver.onCompleted();
    }
}

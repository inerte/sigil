import { createServer } from "node:http";
import { WebSocketServer } from "ws";

function requestPath(request) {
  try {
    return new URL(
      String(request?.url ?? "/"),
      `http://${String(request?.headers?.host ?? "127.0.0.1")}`,
    ).pathname;
  } catch {
    return "/";
  }
}

export async function listenServer(port, routes, onConnection) {
  const routeTable = new Map();
  for (const route of Array.isArray(routes) ? routes : []) {
    const handleName = String(route?.handleName ?? "");
    const path = String(route?.path ?? "");
    if (!handleName || !path) {
      throw new Error("websocket runtime routes require non-empty handleName and path");
    }
    const server = new WebSocketServer({ noServer: true });
    server.on("connection", (socket, request) => {
      onConnection(handleName, socket, request);
    });
    routeTable.set(path, server);
  }

  const server = createServer((_request, response) => {
    response.writeHead(426, {
      "content-type": "text/plain; charset=utf-8",
    });
    response.end("websocket upgrade required");
  });

  server.on("upgrade", (request, socket, head) => {
    const path = requestPath(request);
    const websocketServer = routeTable.get(path);
    if (!websocketServer) {
      socket.write("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
      socket.destroy();
      return;
    }
    websocketServer.handleUpgrade(request, socket, head, (client) => {
      websocketServer.emit("connection", client, request);
    });
  });

  const done = new Promise((resolve, reject) => {
    server.once("close", () => resolve(undefined));
    server.once("error", reject);
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, () => resolve(undefined));
  });

  const address = server.address();
  const assignedPort =
    address && typeof address === "object" && "port" in address
      ? Number(address.port ?? port)
      : Number(port ?? 0);

  return {
    close: async () => {
      for (const websocketServer of routeTable.values()) {
        try {
          websocketServer.close();
        } catch {
          // best-effort cleanup
        }
      }
      await new Promise((resolve) => {
        try {
          server.close(() => resolve(undefined));
        } catch {
          resolve(undefined);
        }
      });
    },
    port: assignedPort,
    wait: () => done,
  };
}

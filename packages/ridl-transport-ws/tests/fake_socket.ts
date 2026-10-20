// A stand-in for the platform `WebSocket`, driven by a test.
//
// It implements the members the session uses, records every message the
// session sends and every `close` it asks for, and lets the test play the
// server: open the connection with a selected subprotocol, deliver a message,
// close the connection, or report an error. Nothing happens on its own: a
// `close` from the session only records the request, and the test completes
// the closing handshake with `serverClose`.

import { encode } from "../mod.ts";
import type { Frame } from "../mod.ts";

/** One `close` the session asked for. */
export interface CloseRequest {
  code?: number;
  reason?: string;
}

export class FakeSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;

  binaryType: BinaryType = "blob";
  protocol = "";
  readyState = FakeSocket.CONNECTING;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  /** Every binary message the session sent, in order. */
  readonly sent: Uint8Array[] = [];
  /** Every `close` the session asked for, in order. */
  readonly closes: CloseRequest[] = [];

  constructor(readonly url: string, readonly protocols: string[]) {}

  send(data: ArrayBufferLike | ArrayBufferView | string | Blob): void {
    if (this.readyState !== FakeSocket.OPEN) {
      throw new Error(`send while readyState is ${this.readyState}`);
    }
    if (data instanceof Uint8Array) {
      this.sent.push(data.slice());
    } else {
      throw new Error("the fake socket accepts Uint8Array messages only");
    }
  }

  close(code?: number, reason?: string): void {
    const request: CloseRequest = {};
    if (code !== undefined) request.code = code;
    if (reason !== undefined) request.reason = reason;
    this.closes.push(request);
    if (this.readyState < FakeSocket.CLOSING) {
      this.readyState = FakeSocket.CLOSING;
    }
  }

  /** The server accepts the connection and selects `protocol`. */
  open(protocol: string): void {
    this.protocol = protocol;
    this.readyState = FakeSocket.OPEN;
    this.onopen?.(new Event("open"));
  }

  /**
   * The server sends one message: a frame is encoded, bytes are sent as a
   * binary message, a string as a text message. Each binary message gets its
   * own `ArrayBuffer`, as the platform gives it.
   */
  deliver(message: Frame | Uint8Array | string): void {
    let data: ArrayBuffer | string;
    if (typeof message === "string") {
      data = message;
    } else {
      const bytes = message instanceof Uint8Array ? message : encode(message);
      data = bytes.slice().buffer;
    }
    this.onmessage?.(new MessageEvent("message", { data }));
  }

  /** The connection closes with `code` and `reason`. */
  serverClose(code: number, reason = ""): void {
    this.readyState = FakeSocket.CLOSED;
    this.onclose?.(new CloseEvent("close", { code, reason }));
  }

  /** The connection fails. */
  error(): void {
    this.onerror?.(new Event("error"));
  }
}

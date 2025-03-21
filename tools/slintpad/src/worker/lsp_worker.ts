// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import slint_init, * as slint_lsp from "@lsp/slint_lsp_wasm.js";
import { InitializeParams, InitializeResult } from "vscode-languageserver";
import {
    createConnection,
    BrowserMessageReader,
    BrowserMessageWriter,
} from "vscode-languageserver/browser";

const port_promise = new Promise<MessagePort>((resolve) => {
    self.onmessage = (event) => {
        const port = event.ports?.[0];
        if (port !== null) {
            resolve(port);
        }
    };
});

slint_init()
    .then(() => port_promise)
    .then((previewer_port) => {
        const reader = new BrowserMessageReader(self);
        const writer = new BrowserMessageWriter(self);

        let the_lsp: slint_lsp.SlintServer;

        const connection = createConnection(reader, writer);

        function send_notification(method: string, params: unknown): boolean {
            connection.sendNotification(method, params);
            return true;
        }

        async function send_request(
            method: string,
            params: unknown,
        ): Promise<unknown> {
            return await connection.sendRequest(method, params);
        }

        async function load_file(path: string): Promise<string> {
            return await connection.sendRequest("slint/load_file", path);
        }

        function highlight(path: string, offset: number) {
            previewer_port.postMessage({
                command: "highlight",
                data: { path: path, offset: offset },
            });
        }

        connection.onInitialize(
            (params: InitializeParams): InitializeResult => {
                the_lsp = slint_lsp.create(
                    params,
                    send_notification,
                    send_request,
                    load_file,
                    highlight,
                );
                const response = the_lsp.server_initialize_result(
                    params.capabilities,
                );
                response.capabilities.codeLensProvider = null; // CodeLenses are not relevant for Slintpad
                return response;
            },
        );

        connection.onRequest(async (method, params, token) => {
            return await the_lsp.handle_request(token, method, params);
        });

        connection.onDidChangeTextDocument(async (param) => {
            await the_lsp.reload_document(
                param.contentChanges[param.contentChanges.length - 1].text,
                param.textDocument.uri,
                param.textDocument.version,
            );
        });

        connection.onDidOpenTextDocument(async (param) => {
            await the_lsp.reload_document(
                param.textDocument.text,
                param.textDocument.uri,
                param.textDocument.version,
            );
        });

        connection.onDidChangeConfiguration(async (_param) => {
            await the_lsp.reload_config();
        });

        // Listen on the connection
        connection.listen();

        // Now that we listen, the client is ready to send the init message
        self.postMessage("OK");
    });

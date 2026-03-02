---
title: First Requests
description: Minimal HTTP and WebSocket examples.
---

## HTTP

```bash
curl http://127.0.0.1:7379/SET/hello/world
curl http://127.0.0.1:7379/GET/hello
```

## JSON WebSocket

```javascript
const ws = new WebSocket('ws://127.0.0.1:7379/.json');
ws.onopen = () => ws.send(JSON.stringify(['SET', 'hello', 'world']));
ws.onmessage = (msg) => console.log(msg.data);
```

## Raw RESP WebSocket

```javascript
const ws = new WebSocket('ws://127.0.0.1:7379/.raw');
ws.onopen = () => ws.send('*1\r\n$4\r\nPING\r\n');
```

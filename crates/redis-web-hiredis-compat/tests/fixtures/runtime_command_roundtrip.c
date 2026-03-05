#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <hiredis/hiredis.h>

static int expect_status(redisReply *reply, const char *expected) {
  if (!reply || !expected) return 0;
  if (reply->type != REDIS_REPLY_STATUS) return 0;
  if (!reply->str) return 0;
  return strcmp(reply->str, expected) == 0;
}

static int expect_string(redisReply *reply, const char *expected) {
  if (!reply || !expected) return 0;
  if (reply->type != REDIS_REPLY_STRING) return 0;
  if (!reply->str) return 0;
  return strcmp(reply->str, expected) == 0;
}

int main(void) {
  const char *host = getenv("REDIS_HOST");
  const char *port_env = getenv("REDIS_PORT");
  int port = port_env ? atoi(port_env) : 6379;
  if (!host) host = "127.0.0.1";

  redisContext *c = redisConnect(host, port);
  if (!c) {
    fprintf(stderr, "redisConnect returned NULL\n");
    return 1;
  }
  if (c->err) {
    fprintf(stderr, "redisConnect error: %s\n", c->errstr);
    redisFree(c);
    return 1;
  }

  redisReply *reply = (redisReply *)redisCommand(c, "PING");
  if (!expect_status(reply, "PONG")) {
    fprintf(stderr, "PING failed\n");
    if (reply) freeReplyObject(reply);
    redisFree(c);
    return 1;
  }
  freeReplyObject(reply);

  reply = (redisReply *)redisCommand(c, "SET compat:key compat:value");
  if (!expect_status(reply, "OK")) {
    fprintf(stderr, "SET failed\n");
    if (reply) freeReplyObject(reply);
    redisFree(c);
    return 1;
  }
  freeReplyObject(reply);

  reply = (redisReply *)redisCommand(c, "GET compat:key");
  if (!expect_string(reply, "compat:value")) {
    fprintf(stderr, "GET failed\n");
    if (reply) freeReplyObject(reply);
    redisFree(c);
    return 1;
  }
  freeReplyObject(reply);

  if (redisAppendCommand(c, "INCR compat:ctr") != REDIS_OK) {
    fprintf(stderr, "redisAppendCommand #1 failed\n");
    redisFree(c);
    return 1;
  }
  if (redisAppendCommand(c, "INCR compat:ctr") != REDIS_OK) {
    fprintf(stderr, "redisAppendCommand #2 failed\n");
    redisFree(c);
    return 1;
  }

  void *out = NULL;
  if (redisGetReply(c, &out) != REDIS_OK || out == NULL) {
    fprintf(stderr, "redisGetReply #1 failed\n");
    redisFree(c);
    return 1;
  }
  freeReplyObject(out);
  out = NULL;
  if (redisGetReply(c, &out) != REDIS_OK || out == NULL) {
    fprintf(stderr, "redisGetReply #2 failed\n");
    redisFree(c);
    return 1;
  }
  freeReplyObject(out);

  redisFree(c);
  return 0;
}

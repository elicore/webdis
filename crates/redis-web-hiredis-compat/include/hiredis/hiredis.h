#ifndef REDIS_WEB_HIREDIS_H
#define REDIS_WEB_HIREDIS_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define REDIS_OK 0
#define REDIS_ERR -1

#define REDIS_REPLY_STRING 1
#define REDIS_REPLY_ARRAY 2
#define REDIS_REPLY_INTEGER 3
#define REDIS_REPLY_NIL 4
#define REDIS_REPLY_STATUS 5
#define REDIS_REPLY_ERROR 6

typedef struct redisContext {
  int err;
  char errstr[128];
  int fd;
  int flags;
  void *obuf;
  void *reader;
  int connection_type;
  void *private_data;
} redisContext;

typedef struct redisReply {
  int type;
  long long integer;
  size_t len;
  char *str;
  size_t elements;
  struct redisReply **element;
} redisReply;

redisContext *redisConnect(const char *ip, int port);
redisContext *redisConnectWithTimeout(const char *ip, int port, void *timeout);
redisContext *redisConnectUnix(const char *path);
int redisReconnect(redisContext *c);
void redisFree(redisContext *c);

void *redisCommand(redisContext *c, const char *format);
void *redisvCommand(redisContext *c, const char *format, void *ap);
void *redisCommandArgv(redisContext *c, int argc, const char **argv, const size_t *argvlen);

int redisAppendCommand(redisContext *c, const char *format);
int redisAppendCommandArgv(redisContext *c, int argc, const char **argv, const size_t *argvlen);
int redisGetReply(redisContext *c, void **reply);

void freeReplyObject(void *reply);

void *redisAsyncConnect(const char *ip, int port);
void *redisAsyncConnectUnix(const char *path);
int redisInitiateSSLWithContext(redisContext *c, void *ssl);
void redisInitOpenSSL(void);

#ifdef __cplusplus
}
#endif

#endif

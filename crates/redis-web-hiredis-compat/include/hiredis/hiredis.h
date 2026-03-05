#ifndef REDIS_WEB_COMPAT_HIREDIS_H
#define REDIS_WEB_COMPAT_HIREDIS_H

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/time.h>

#include <hiredis/alloc.h>
#include <hiredis/read.h>
#include <hiredis/sds.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct redisReply {
  int type;
  long long integer;
  double dval;
  size_t len;
  char *str;
  char vtype[4];
  size_t elements;
  struct redisReply **element;
} redisReply;

typedef int redisFD;
typedef void(redisPushFn)(void *, void *);
typedef void(redisAsyncPushFn)(void *, void *);

typedef struct redisContext {
  int err;
  char errstr[128];
  redisFD fd;
  int flags;
  void *obuf;
  void *reader;
  int connection_type;
  void *private_data;
} redisContext;

typedef struct redisOptions {
  int type;
  int options;
  const struct timeval *connect_timeout;
  const struct timeval *command_timeout;
  union {
    struct {
      const char *source_addr;
      const char *ip;
      int port;
    } tcp;
    const char *unix_socket;
    redisFD fd;
  } endpoint;
  void *privdata;
  void (*free_privdata)(void *);
  redisPushFn *push_cb;
  redisAsyncPushFn *async_push_cb;
} redisOptions;

#define redisIsPushReply(r) (((redisReply *)(r))->type == REDIS_REPLY_PUSH)

redisContext *redisConnectWithOptions(const redisOptions *options);
redisContext *redisConnect(const char *ip, int port);
redisContext *redisConnectWithTimeout(const char *ip, int port, const struct timeval tv);
redisContext *redisConnectNonBlock(const char *ip, int port);
redisContext *redisConnectBindNonBlock(const char *ip, int port, const char *source_addr);
redisContext *redisConnectBindNonBlockWithReuse(const char *ip, int port, const char *source_addr);
redisContext *redisConnectUnix(const char *path);
redisContext *redisConnectUnixWithTimeout(const char *path, const struct timeval tv);
redisContext *redisConnectUnixNonBlock(const char *path);
redisContext *redisConnectFd(redisFD fd);
int redisReconnect(redisContext *c);

redisPushFn *redisSetPushCallback(redisContext *c, redisPushFn *fn);
int redisSetTimeout(redisContext *c, const struct timeval tv);
int redisEnableKeepAlive(redisContext *c);
int redisEnableKeepAliveWithInterval(redisContext *c, int interval);
int redisSetTcpUserTimeout(redisContext *c, unsigned int timeout);
void redisFree(redisContext *c);
redisFD redisFreeKeepFd(redisContext *c);
int redisBufferRead(redisContext *c);
int redisBufferWrite(redisContext *c, int *done);
int redisGetReply(redisContext *c, void **reply);
int redisGetReplyFromReader(redisContext *c, void **reply);

int redisAppendFormattedCommand(redisContext *c, const char *cmd, size_t len);
int redisvAppendCommand(redisContext *c, const char *format, va_list ap);
int redisAppendCommand(redisContext *c, const char *format, ...);
int redisAppendCommandArgv(redisContext *c, int argc, const char **argv, const size_t *argvlen);

void *redisvCommand(redisContext *c, const char *format, va_list ap);
void *redisCommand(redisContext *c, const char *format, ...);
void *redisCommandArgv(redisContext *c, int argc, const char **argv, const size_t *argvlen);

void freeReplyObject(void *reply);

int redisvFormatCommand(char **target, const char *format, va_list ap);
int redisFormatCommand(char **target, const char *format, ...);
long long redisFormatCommandArgv(char **target, int argc, const char **argv, const size_t *argvlen);
long long redisFormatSdsCommandArgv(sds *target, int argc, const char **argv, const size_t *argvlen);
void redisFreeCommand(char *cmd);
void redisFreeSdsCommand(sds cmd);

void *redisAsyncConnectWithOptions(const redisOptions *options);
void *redisAsyncConnect(const char *ip, int port);
void *redisAsyncConnectBind(const char *ip, int port, const char *source_addr);
void *redisAsyncConnectBindWithReuse(const char *ip, int port, const char *source_addr);
void *redisAsyncConnectUnix(const char *path);
int redisAsyncSetConnectCallback(void *ac, void *fn);
int redisAsyncSetConnectCallbackNC(void *ac, void *fn);
int redisAsyncSetDisconnectCallback(void *ac, void *fn);
void *redisAsyncSetPushCallback(void *ac, void *fn);
int redisAsyncSetTimeout(void *ac, struct timeval tv);
void redisAsyncDisconnect(void *ac);
void redisAsyncFree(void *ac);
void redisAsyncHandleRead(void *ac);
void redisAsyncHandleWrite(void *ac);
void redisAsyncHandleTimeout(void *ac);
void redisAsyncRead(void *ac);
void redisAsyncWrite(void *ac);
void redisProcessCallbacks(void *ac);
int redisvAsyncCommand(void *ac, void *fn, void *privdata, const char *format, va_list ap);
int redisAsyncCommand(void *ac, void *fn, void *privdata, const char *format, ...);
int redisAsyncCommandArgv(void *ac, void *fn, void *privdata, int argc, const char **argv, const size_t *argvlen);
int redisAsyncFormattedCommand(void *ac, void *fn, void *privdata, const char *cmd, size_t len);

int redisCheckConnectDone(redisContext *c, int *completed);
int redisCheckSocketError(redisContext *c);
int redisContextConnectTcp(redisContext *c, const char *addr, int port, const struct timeval *timeout);
int redisContextConnectBindTcp(redisContext *c, const char *addr, int port, const struct timeval *timeout, const char *source_addr);
int redisContextConnectUnix(redisContext *c, const char *path, const struct timeval *timeout);
int redisContextSetTimeout(redisContext *c, const struct timeval tv);
int redisContextSetTcpUserTimeout(redisContext *c, unsigned int timeout);
int redisContextUpdateConnectTimeout(redisContext *c, const struct timeval *timeout);
int redisContextUpdateCommandTimeout(redisContext *c, const struct timeval *timeout);
int redisKeepAlive(redisContext *c, int interval);
int redisSetTcpNoDelay(redisContext *c);
void redisNetClose(redisContext *c);
ssize_t redisNetRead(redisContext *c, char *buf, size_t bufcap);
ssize_t redisNetWrite(redisContext *c);

int redisInitiateSSLWithContext(redisContext *c, void *ssl);
void redisInitOpenSSL(void);

#ifdef __cplusplus
}
#endif

#endif

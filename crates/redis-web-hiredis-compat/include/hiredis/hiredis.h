#ifndef REDIS_WEB_COMPAT_HIREDIS_H
#define REDIS_WEB_COMPAT_HIREDIS_H

#include <stdarg.h>
#include <stddef.h>

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

int redisvFormatCommand(char **target, const char *format, va_list ap);
int redisFormatCommand(char **target, const char *format, ...);
long long redisFormatCommandArgv(char **target, int argc, const char **argv, const size_t *argvlen);
long long redisFormatSdsCommandArgv(sds *target, int argc, const char **argv, const size_t *argvlen);
void redisFreeCommand(char *cmd);
void redisFreeSdsCommand(sds cmd);

void *redisAsyncConnect(const char *ip, int port);
void *redisAsyncConnectUnix(const char *path);
int redisInitiateSSLWithContext(redisContext *c, void *ssl);
void redisInitOpenSSL(void);

#ifdef __cplusplus
}
#endif

#endif

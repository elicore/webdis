#ifndef REDIS_WEB_COMPAT_SDS_H
#define REDIS_WEB_COMPAT_SDS_H

#include <stddef.h>
#include <sys/types.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef char *sds;

sds sdsnewlen(const void *init, size_t initlen);
sds sdsnew(const char *init);
sds sdsempty(void);
sds sdsdup(const sds s);
void sdsfree(sds s);
sds sdscpylen(sds s, const char *t, size_t len);
sds sdscpy(sds s, const char *t);
void sdsfreesplitres(sds *tokens, int count);

void *sds_malloc(size_t size);
void *sds_realloc(void *ptr, size_t size);
void sds_free(void *ptr);

#ifdef __cplusplus
}
#endif

#endif

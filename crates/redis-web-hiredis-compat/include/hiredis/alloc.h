#ifndef REDIS_WEB_COMPAT_ALLOC_H
#define REDIS_WEB_COMPAT_ALLOC_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct hiredisAllocFuncs {
  void *(*mallocFn)(size_t);
  void *(*callocFn)(size_t, size_t);
  void *(*reallocFn)(void *, size_t);
  char *(*strdupFn)(const char *);
  void (*freeFn)(void *);
} hiredisAllocFuncs;

extern hiredisAllocFuncs hiredisAllocFns;

hiredisAllocFuncs hiredisSetAllocators(hiredisAllocFuncs *ha);
void hiredisResetAllocators(void);

void *hi_malloc(size_t size);
void *hi_calloc(size_t nmemb, size_t size);
void *hi_realloc(void *ptr, size_t size);
char *hi_strdup(const char *str);
void hi_free(void *ptr);

#ifdef __cplusplus
}
#endif

#endif

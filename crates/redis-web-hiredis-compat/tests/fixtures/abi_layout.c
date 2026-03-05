#include <stddef.h>
#include <hiredis/hiredis.h>
#include <hiredis/read.h>

_Static_assert(sizeof(redisReply) >= sizeof(int) + sizeof(long long), "redisReply layout changed unexpectedly");
_Static_assert(offsetof(redisReply, type) == 0, "redisReply.type offset mismatch");
_Static_assert(offsetof(redisReply, integer) > offsetof(redisReply, type), "redisReply.integer offset mismatch");

_Static_assert(sizeof(redisContext) >= 64, "redisContext too small for upstream-compatible runtime use");
_Static_assert(offsetof(redisContext, err) > 0, "redisContext.err offset mismatch");
_Static_assert(offsetof(redisContext, errstr) > offsetof(redisContext, err), "redisContext.errstr offset mismatch");

_Static_assert(sizeof(redisReader) >= 64, "redisReader too small");
_Static_assert(offsetof(redisReader, err) == 0, "redisReader.err offset mismatch");

int main(void) {
  return 0;
}

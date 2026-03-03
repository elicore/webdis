#include <hiredis/hiredis.h>

int main(void) {
  redisContext *c = redisConnect("127.0.0.1", 6379);
  if (!c) {
    return 1;
  }

  void *reply = redisCommand(c, "PING");
  if (reply) {
    freeReplyObject(reply);
  }

  redisFree(c);
  return 0;
}

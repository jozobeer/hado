#ifndef SHIN_RUNTIME_H
#define SHIN_RUNTIME_H
#include <stddef.h>
#include <stdint.h>

typedef struct {
  size_t length;
  unsigned char bytes[];
} ShinString;

void *shin_alloc(size_t size);
int64_t shin_string_length(const ShinString *string);
ShinString *shin_string_concat(const ShinString *left, const ShinString *right);
int64_t shin_string_compare(const ShinString *left, const ShinString *right);
#endif

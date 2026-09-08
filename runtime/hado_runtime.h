#ifndef HADO_RUNTIME_H
#define HADO_RUNTIME_H
#include <stddef.h>
#include <stdint.h>

typedef struct {
  size_t length;
  unsigned char bytes[];
} HadoString;

void *hado_alloc(size_t size);
int64_t hado_string_length(const HadoString *string);
HadoString *hado_string_concat(const HadoString *left, const HadoString *right);
int64_t hado_string_compare(const HadoString *left, const HadoString *right);
#endif

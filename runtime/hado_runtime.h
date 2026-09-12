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

/* Bytes shares the HadoString layout with String. length counts bytes here,
   while hado_string_length counts Unicode scalar values. */
int64_t hado_bytes_length(const HadoString *bytes);
int64_t hado_bytes_get(const HadoString *bytes, int64_t index);
int8_t hado_bytes_is_utf8(const HadoString *bytes);
HadoString *hado_bytes_display(const HadoString *bytes);
HadoString *hado_bytes_of_string(const HadoString *value);
HadoString *hado_bytes_to_string_unchecked(const HadoString *bytes);
HadoString *hado_bytes_alloc(int64_t length);
HadoString *hado_bytes_fill_unchecked(HadoString *bytes, int64_t index, int64_t value);
#endif

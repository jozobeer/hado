#include "../../runtime/hado_runtime.h"
#include <stdlib.h>

HadoString *hado_char_to_string(uint32_t value);

static unsigned char escape(unsigned char byte, unsigned char quote) {
  if (byte == '\n') return 'n';
  if (byte == '\t') return 't';
  if (byte == '\\' || byte == quote) return byte;
  return 0;
}

static HadoString *quote_text(const HadoString *value, unsigned char quote) {
  if (value->length > (SIZE_MAX - sizeof(HadoString) - 2) / 2) abort();
  HadoString *result = hado_alloc(sizeof(HadoString) + 2 * value->length + 2);
  if (result == NULL) abort();
  size_t length = 0;
  result->bytes[length++] = quote;
  for (size_t i = 0; i < value->length; ++i) {
    unsigned char escaped = escape(value->bytes[i], quote);
    if (escaped) result->bytes[length++] = '\\';
    result->bytes[length++] = escaped ? escaped : value->bytes[i];
  }
  result->bytes[length++] = quote;
  result->length = length;
  result->scalars = hado_scalar_count(result->bytes, length);
  return result;
}

HadoString *hado_quote_string(const HadoString *value) {
  return quote_text(value, '"');
}

HadoString *hado_quote_char(uint32_t value) {
  return quote_text(hado_char_to_string(value), '\'');
}

size_t hado_format_float(double value, unsigned char *bytes, size_t capacity);

HadoString *hado_canonical_float(double value) {
  unsigned char bytes[400];
  size_t length = hado_format_float(value, bytes, sizeof(bytes));
  if (length == 0 || length > sizeof(bytes)) abort();
  HadoString *result = hado_alloc(sizeof(HadoString) + length);
  if (result == NULL) abort();
  result->length = length;
  result->scalars = length;
  for (size_t i = 0; i < length; ++i) result->bytes[i] = bytes[i];
  return result;
}

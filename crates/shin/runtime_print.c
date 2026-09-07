#include "../../runtime/shin_runtime.h"
#include <stdlib.h>

ShinString *shin_char_to_string(uint32_t value);

static unsigned char escape(unsigned char byte, unsigned char quote) {
  if (byte == '\n') return 'n';
  if (byte == '\t') return 't';
  if (byte == '\\' || byte == quote) return byte;
  return 0;
}

static ShinString *quote_text(const ShinString *value, unsigned char quote) {
  if (value->length > (SIZE_MAX - sizeof(ShinString) - 2) / 2) abort();
  ShinString *result = shin_alloc(sizeof(ShinString) + 2 * value->length + 2);
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
  return result;
}

ShinString *shin_quote_string(const ShinString *value) {
  return quote_text(value, '"');
}

ShinString *shin_quote_char(uint32_t value) {
  return quote_text(shin_char_to_string(value), '\'');
}

size_t shin_format_float(double value, unsigned char *bytes, size_t capacity);

ShinString *shin_canonical_float(double value) {
  unsigned char bytes[400];
  size_t length = shin_format_float(value, bytes, sizeof(bytes));
  if (length == 0 || length > sizeof(bytes)) abort();
  ShinString *result = shin_alloc(sizeof(ShinString) + length);
  if (result == NULL) abort();
  result->length = length;
  for (size_t i = 0; i < length; ++i) result->bytes[i] = bytes[i];
  return result;
}

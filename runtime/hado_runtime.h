#ifndef HADO_RUNTIME_H
#define HADO_RUNTIME_H
#include <stddef.h>
#include <stdint.h>

/* String and Bytes share this layout (spec §19, §55). `length` counts bytes.
   `scalars` counts Unicode scalar values for a String; a Bytes value that has
   not been through Bytes.to_string leaves it at HADO_SCALARS_UNKNOWN, because
   counting the scalars of an arbitrary byte string is only meaningful once
   RFC 3629 well-formedness is known. */
#define HADO_SCALARS_UNKNOWN ((size_t)-1)

typedef struct {
  size_t length;
  size_t scalars;
  unsigned char bytes[];
} HadoString;

typedef struct {
  int64_t count;
  uint64_t slots[];
} HadoArray;

HadoArray *hado_array_alloc(int64_t count);
int64_t hado_array_length(const HadoArray *array);

void *hado_alloc(size_t size);
int64_t hado_argv_count(int64_t ignored);
HadoString *hado_argv_get(int64_t index);
int8_t hado_env_has(const HadoString *name);
HadoString *hado_env_get(const HadoString *name);
HadoString *hado_cwd(int64_t ignored);
int64_t hado_io_read_file(const HadoString *path);
int64_t hado_io_write_file(const HadoString *path, const HadoString *contents);
int64_t hado_io_log(const HadoString *contents);
int64_t hado_io_read_dir(const HadoString *path);
int64_t hado_io_read_stdin(int64_t ignored);
HadoString *hado_io_take_bytes(int64_t ignored);
int64_t hado_io_dir_count(int64_t ignored);
HadoString *hado_io_dir_get(int64_t index);
int8_t hado_int_parse_is_valid(const HadoString *string);
int64_t hado_int_parse_unchecked(const HadoString *string);
int64_t hado_string_length(const HadoString *string);
HadoString *hado_string_concat(const HadoString *left, const HadoString *right);
int64_t hado_string_compare(const HadoString *left, const HadoString *right);
HadoString *hado_string_slice(const HadoString *string, int64_t start, int64_t end);
uint32_t hado_string_get(const HadoString *string, int64_t index);
int64_t hado_string_find(const HadoString *string, const HadoString *separator, int64_t from);
HadoString *hado_string_trim(const HadoString *string);
HadoString *hado_string_split(const HadoString *string, const HadoString *separator);
HadoString *hado_string_join(const HadoArray *values, const HadoString *separator);

/* Counts the Unicode scalar values in `bytes`. Exposed so the printer in
   crates/hado/runtime_print.c fills the same header, and counted by
   hado_byte_scans so a gate can tell a scan from a constant-time read. */
size_t hado_scalar_count(const unsigned char *bytes, size_t length);

/* §54's canonical Float spelling. Defined by the Rust formatter in
   crates/hado/float_format.rs and shared by Float.to_string and the printer,
   so both print the same shortest round-tripping decimal. Returns 0 when the
   result does not fit in `capacity`. */
size_t hado_format_float(double value, unsigned char *bytes, size_t capacity);
extern size_t hado_byte_scans;

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

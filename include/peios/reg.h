#ifndef PEIOS_REG_H
#define PEIOS_REG_H
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* All spans are borrowed for a call. Non-null pointers must be readable for
 * their stated lengths. Strings are UTF-8 without a required terminator.
 * Returned spans belong to their immutable handle and expire when it is freed.
 * Read-only calls can run concurrently; freeing requires exclusive ownership.
 * Every returned handle must be freed by its matching function. Handles from
 * failed operations also require freeing. Null destruction is harmless.
 * Error accessors return zero on success or a POSIX error number on failure.
 * A null input handle has error EINVAL. */
typedef struct {
    const uint8_t *data;
    size_t len;
} peios_reg_bytes;
typedef struct peios_reg_document peios_reg_document;
typedef struct peios_reg_apply peios_reg_apply;

uint32_t peios_reg_abi_version(void);
peios_reg_document *peios_reg_document_parse(peios_reg_bytes json);
peios_reg_document *peios_reg_document_export(peios_reg_bytes path);
/* A {NULL, 0} layer selects the default. An empty non-null layer is explicit. */
peios_reg_apply *peios_reg_document_apply(const peios_reg_document *, peios_reg_bytes layer);
peios_reg_bytes peios_reg_document_json(const peios_reg_document *);
void peios_reg_document_free(peios_reg_document *);
int32_t peios_reg_document_error(const peios_reg_document *);
peios_reg_bytes peios_reg_document_message(const peios_reg_document *);
/* Read the count only after checking the error accessor. */
size_t peios_reg_apply_count(const peios_reg_apply *);
void peios_reg_apply_free(peios_reg_apply *);
int32_t peios_reg_apply_error(const peios_reg_apply *);
peios_reg_bytes peios_reg_apply_message(const peios_reg_apply *);

#ifdef __cplusplus
}
#endif
#endif

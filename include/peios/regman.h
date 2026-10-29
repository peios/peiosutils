#ifndef PEIOS_REGMAN_H
#define PEIOS_REGMAN_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

/* Non-null spans must be readable for their lengths. Inputs are borrowed only
 * during calls. Strings are UTF-8; filesystem paths are raw Unix bytes.
 * {NULL, 0} is absent; a non-null zero-length span is present and empty.
 * Handles are immutable. Returned views expire when their handle is freed.
 * Reading can be concurrent; destruction requires exclusive ownership.
 * Failed operations return handles that also require freeing. */
typedef struct {
    const uint8_t *data;
    size_t len;
} peios_regman_bytes;
typedef struct peios_regman_result peios_regman_result;
typedef struct {
    peios_regman_bytes provider;
    peios_regman_bytes file;
    peios_regman_bytes anchor;
    peios_regman_bytes canonical;
    peios_regman_bytes type_name;
    peios_regman_bytes default_value;
    peios_regman_bytes valid;
    peios_regman_bytes applies;
    peios_regman_bytes deprecated;
    peios_regman_bytes body;
    uint64_t fence_offset;
} peios_regman_record_view;
typedef struct {
    uint64_t line;
    peios_regman_bytes message;
} peios_regman_issue_view;

enum { PEIOS_REGMAN_EXACT = 0, PEIOS_REGMAN_KEY = 1, PEIOS_REGMAN_SEARCH = 2 };
uint32_t peios_regman_abi_version(void);
/* Parsing returns valid records alongside recoverable parse issues. */
peios_regman_result *peios_regman_parse(peios_regman_bytes source);
/* Absent directory/index use REGMAN_DIR/REGMAN_INDEX or their built-in defaults.
 * Queries are case-folded internally. SEARCH splits text on whitespace. */
peios_regman_result *peios_regman_query(peios_regman_bytes directory, peios_regman_bytes index,
                                        uint32_t mode, peios_regman_bytes text);
/* Case-folds UTF-8 text as manual names compare, into out; stores the folded
 * length and returns 0, ERANGE with the length needed, or EINVAL. */
int32_t peios_regman_fold(peios_regman_bytes text, uint8_t *out, size_t capacity, size_t *length);
void peios_regman_free(peios_regman_result *);
int32_t peios_regman_error(const peios_regman_result *);
peios_regman_bytes peios_regman_message(const peios_regman_result *);
size_t peios_regman_record_count(const peios_regman_result *);
size_t peios_regman_issue_count(const peios_regman_result *);
/* Getters return zero on success; errors are POSIX codes and leave output untouched. */
int32_t peios_regman_record(const peios_regman_result *, size_t index, peios_regman_record_view *);
int32_t peios_regman_issue(const peios_regman_result *, size_t index, peios_regman_issue_view *);
#ifdef __cplusplus
}
#endif
#endif

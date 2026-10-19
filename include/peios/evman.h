#ifndef PEIOS_EVMAN_H
#define PEIOS_EVMAN_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

/* All inputs are borrowed for a call. Non-null pointers must be live and
 * readable for the stated lengths; arrays and output slots must be aligned.
 * Strings are UTF-8. Paths contain raw Unix bytes. {NULL, 0} denotes absence.
 * Handles own immutable results; views expire when their handle is freed.
 * Concurrent reads are allowed; destruction requires exclusive ownership.
 * Failed operations return handles which also require freeing. Null free is safe.
 * Error accessors return zero or a POSIX error number; null has error EINVAL.
 * Indexed getters return a POSIX code and leave output untouched on failure. */
typedef struct {
    const uint8_t *data;
    size_t len;
} peios_evman_bytes;
typedef struct peios_evman_catalog peios_evman_catalog;
typedef struct peios_evman_matches peios_evman_matches;
typedef struct {
    peios_evman_bytes path;
    peios_evman_bytes text;
} peios_evman_source;
typedef struct {
    uint32_t kind;
    peios_evman_bytes name;
    peios_evman_bytes file;
    peios_evman_bytes provider;
    peios_evman_bytes body;
    uint64_t line;
} peios_evman_record_view;
typedef struct {
    peios_evman_bytes file;
    uint64_t line;
    uint32_t rule;
    peios_evman_bytes message;
} peios_evman_finding_view;
typedef struct {
    peios_evman_bytes key;
    peios_evman_bytes value;
    uint64_t line;
} peios_evman_header_view;
typedef struct {
    uint32_t kind;
    peios_evman_bytes name;
    peios_evman_bytes presence;
    peios_evman_bytes values;
    peios_evman_bytes closed;
    uint64_t line;
    /* Event-local Markdown explanation; absent for group includes. */
    peios_evman_bytes gloss;
} peios_evman_line_view;

enum { PEIOS_EVMAN_LOOKUP = 0, PEIOS_EVMAN_SEARCH = 1, PEIOS_EVMAN_ALL = 2 };
enum { PEIOS_EVMAN_FIELD = 1, PEIOS_EVMAN_GROUP = 2, PEIOS_EVMAN_EVENT = 3 };
enum {
    PEIOS_EVMAN_MATCH_EVENT = 1,
    PEIOS_EVMAN_MATCH_FIELD = 2,
    PEIOS_EVMAN_MATCH_VARIANT = 3,
    PEIOS_EVMAN_MATCH_ATTRIBUTE = 4,
    PEIOS_EVMAN_MATCH_INDEX = 5,
    PEIOS_EVMAN_MATCH_SEARCH = 6,
    PEIOS_EVMAN_MATCH_ALL = 7
};
uint32_t peios_evman_abi_version(void);
/* An absent directory selects EVMAN_DIR or /usr/share/evman. */
peios_evman_catalog *peios_evman_load(peios_evman_bytes directory);
/* Source paths identify providers. Parse and cross-source validation findings
 * accompany a usable catalog; inspect them separately from operation errors. */
peios_evman_catalog *peios_evman_parse(const peios_evman_source *, size_t count);
/* Query results own copies and can outlive the catalog. SEARCH splits text on whitespace. */
peios_evman_matches *peios_evman_query(const peios_evman_catalog *, uint32_t mode,
                                       peios_evman_bytes text);
void peios_evman_catalog_free(peios_evman_catalog *);
int32_t peios_evman_catalog_error(const peios_evman_catalog *);
peios_evman_bytes peios_evman_catalog_message(const peios_evman_catalog *);
void peios_evman_matches_free(peios_evman_matches *);
int32_t peios_evman_matches_error(const peios_evman_matches *);
peios_evman_bytes peios_evman_matches_message(const peios_evman_matches *);
size_t peios_evman_finding_count(const peios_evman_catalog *);
/* Rule zero denotes a format issue; other values are numbered catalog rules. */
int32_t peios_evman_finding(const peios_evman_catalog *, size_t index, peios_evman_finding_view *);
size_t peios_evman_record_count(const peios_evman_matches *);
uint32_t peios_evman_match_kind(const peios_evman_matches *);
int32_t peios_evman_record(const peios_evman_matches *, size_t index, peios_evman_record_view *);
peios_evman_bytes peios_evman_qualifier(const peios_evman_matches *);
peios_evman_bytes peios_evman_attribute(const peios_evman_matches *);
peios_evman_bytes peios_evman_prefix(const peios_evman_matches *);
size_t peios_evman_header_count(const peios_evman_matches *, size_t record);
int32_t peios_evman_header(const peios_evman_matches *, size_t record, size_t index,
                           peios_evman_header_view *);
size_t peios_evman_include_count(const peios_evman_matches *, size_t record);
int32_t peios_evman_include(const peios_evman_matches *, size_t record, size_t index,
                            peios_evman_line_view *);
size_t peios_evman_line_count(const peios_evman_matches *, size_t record);
int32_t peios_evman_line(const peios_evman_matches *, size_t record, size_t index,
                         peios_evman_line_view *);
/* Line kinds use FIELD=1 and INCLUDE=2. The group-include getter returns absent
 * presence, values and closed fields. Event lines have present (possibly empty)
 * presence and optional values/closed fields. Empty differs from absent. */
#ifdef __cplusplus
}
#endif
#endif

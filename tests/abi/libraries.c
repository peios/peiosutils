#ifdef NDEBUG
#undef NDEBUG
#endif

#include <assert.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "peios/reg.h"
#include "peios/regman.h"
#include "peios/evman.h"

#define TEXT(value) {(const uint8_t *)(value), sizeof(value) - 1}

static void registry_documents(void) {
    assert(peios_reg_abi_version() == 1);
    peios_reg_document *document = peios_reg_document_parse((peios_reg_bytes)TEXT("{\"keys\":[]}"));
    assert(peios_reg_document_error(document) == 0);
    peios_reg_bytes json = peios_reg_document_json(document);
    assert(json.len == 11 && memcmp(json.data, "{\"keys\":[]}", 11) == 0);
    peios_reg_document_free(document);

    document = peios_reg_document_parse((peios_reg_bytes){NULL, 1});
    assert(peios_reg_document_error(document) == EINVAL);
    assert(peios_reg_document_message(document).len != 0);
    peios_reg_document_free(document);
    document = peios_reg_document_parse((peios_reg_bytes){(const uint8_t *)1, SIZE_MAX});
    assert(peios_reg_document_error(document) == EINVAL);
    peios_reg_document_free(document);
    peios_reg_document_free(NULL);
    peios_reg_apply_free(NULL);
    assert(peios_reg_document_error(NULL) == EINVAL);
}

static void registry_manual(void) {
    assert(peios_regman_abi_version() == 1);
    char input[] = "--- machine\\example\ncanonical: Machine\\Example\n\nExample.\n";
    peios_regman_result *manual =
        peios_regman_parse((peios_regman_bytes){(const uint8_t *)input, strlen(input)});
    memset(input, 'x', sizeof(input));
    assert(peios_regman_error(manual) == 0);
    assert(peios_regman_record_count(manual) == 1);
    assert(peios_regman_issue_count(manual) == 0);
    peios_regman_record_view record;
    assert(peios_regman_record(manual, 0, &record) == 0);
    assert(record.canonical.len == 15);
    assert(memcmp(record.canonical.data, "Machine\\Example", 15) == 0);
    assert(record.type_name.data == NULL);
    assert(record.body.len == 8 && memcmp(record.body.data, "Example.", 8) == 0);
    peios_regman_record_view previous = record;
    assert(peios_regman_record(manual, SIZE_MAX, &record) == ERANGE);
    assert(previous.canonical.data == record.canonical.data);
    assert(previous.canonical.len == record.canonical.len);
    assert(previous.body.data == record.body.data);
    peios_regman_free(manual);

    const uint8_t invalid_utf8[] = {0xff};
    manual = peios_regman_parse((peios_regman_bytes){invalid_utf8, sizeof(invalid_utf8)});
    assert(peios_regman_error(manual) == EINVAL);
    peios_regman_free(manual);
    peios_regman_free(NULL);
}

static void event_catalog(void) {
    assert(peios_evman_abi_version() == 1);
    peios_evman_source source = {
        TEXT("example.evman"), TEXT("--- field example.value\ntype: invalid\n\nExample value.\n")};
    peios_evman_catalog *catalog = peios_evman_parse(&source, 1);
    assert(peios_evman_catalog_error(catalog) == 0);
    assert(peios_evman_finding_count(catalog) != 0);
    peios_evman_finding_view finding;
    assert(peios_evman_finding(catalog, 0, &finding) == 0);
    assert(finding.file.len == 13 && finding.line != 0 && finding.message.len != 0);
    peios_evman_matches *matches =
        peios_evman_query(catalog, PEIOS_EVMAN_LOOKUP, (peios_evman_bytes)TEXT("example.value"));
    assert(peios_evman_matches_error(matches) == 0);
    peios_evman_catalog_free(catalog);
    assert(peios_evman_match_kind(matches) == PEIOS_EVMAN_MATCH_FIELD);
    assert(peios_evman_record_count(matches) == 1);
    peios_evman_record_view record;
    assert(peios_evman_record(matches, 0, &record) == 0);
    assert(record.kind == PEIOS_EVMAN_FIELD);
    assert(record.name.len == 13 && memcmp(record.name.data, "example.value", 13) == 0);
    assert(peios_evman_header_count(matches, 0) == 1);
    peios_evman_header_view header;
    assert(peios_evman_header(matches, 0, 0, &header) == 0);
    assert(header.key.len == 4 && memcmp(header.key.data, "type", 4) == 0);
    assert(header.value.len == 7 && memcmp(header.value.data, "invalid", 7) == 0);
    assert(peios_evman_header(matches, 0, 1, &header) == ERANGE);
    peios_evman_matches_free(matches);
    catalog = peios_evman_parse(NULL, 1);
    assert(peios_evman_catalog_error(catalog) == EINVAL);
    peios_evman_catalog_free(catalog);
    peios_evman_catalog_free(NULL);
    peios_evman_matches_free(NULL);
}

static void event_line_gloss(void) {
    char input[] = "--- group example\ninclude: example.value\n\nA group.\n\n"
                   "--- event example.updated\ntier: standard\n\nAn update.\n\n"
                   "include: example\nfield: example.value optional\n"
                   "  First paragraph.\n\n  Second paragraph.\n"
                   "field: example.value optional\n";
    peios_evman_source source = {TEXT("example.evman"), {(const uint8_t *)input, strlen(input)}};
    peios_evman_catalog *catalog = peios_evman_parse(&source, 1);
    assert(peios_evman_catalog_error(catalog) == 0);
    memset(input, 'x', sizeof(input));
    peios_evman_matches *event =
        peios_evman_query(catalog, PEIOS_EVMAN_LOOKUP, (peios_evman_bytes)TEXT("example.updated"));
    peios_evman_matches *group =
        peios_evman_query(catalog, PEIOS_EVMAN_LOOKUP, (peios_evman_bytes)TEXT("example"));
    assert(peios_evman_matches_error(event) == 0);
    assert(peios_evman_matches_error(group) == 0);
    peios_evman_catalog_free(catalog);
    peios_evman_line_view line;
    assert(peios_evman_line(event, 0, 0, &line) == 0);
    assert(line.gloss.data != NULL && line.gloss.len == 0);
    assert(line.presence.data != NULL && line.presence.len == 0);
    assert(peios_evman_line(event, 0, 1, &line) == 0);
    const char expected[] = "First paragraph.\n\nSecond paragraph.";
    assert(line.gloss.len == sizeof(expected) - 1);
    assert(memcmp(line.gloss.data, expected, line.gloss.len) == 0);
    peios_evman_bytes previous = line.gloss;
    assert(peios_evman_line(event, 0, SIZE_MAX, &line) == ERANGE);
    assert(line.gloss.data == previous.data && line.gloss.len == previous.len);
    assert(peios_evman_line(event, 0, 2, &line) == 0);
    assert(line.gloss.data != NULL && line.gloss.len == 0);
    assert(peios_evman_include(group, 0, 0, &line) == 0);
    assert(line.gloss.data == NULL && line.gloss.len == 0);
    assert(line.presence.data == NULL && line.presence.len == 0);
    assert(line.values.data == NULL && line.closed.data == NULL);
    peios_evman_matches_free(event);
    peios_evman_matches_free(group);
}

int main(void) {
    registry_documents();
    registry_manual();
    event_catalog();
    event_line_gloss();
    puts("utility shared-library ABI checks passed");
    return 0;
}

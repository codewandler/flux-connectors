op airtable-record-create(base_id: String, table_id: String, cell_values: Any) -> Any
  description "Create one record in a table. Cell values are supplied under `fields`, keyed by column name, and must already be in the exact JSON form each column expects — Airtable's `typecast` coercion cannot be requested yet (see the connector's notes). The created record, with its `rec…` id, is in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://api.airtable.com"
  $url = fmt("{base}/v0/{base_id}/{table_id}")
  $content_type = "application/json"
  $payload = { fields: $cell_values }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response

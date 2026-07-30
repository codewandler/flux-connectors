op airtable-record-update(base_id: String, table_id: String, record_id: String, cell_values: Any) -> Any
  description "Update one record's cell values. The write is sparse: only the columns named in `fields` change and every other column of the record is left exactly as it was. Values must already be in the form each column expects — `typecast` coercion cannot be requested yet. The updated record is in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.airtable.com"
  url = fmt("{base}/v0/{base_id}/{table_id}/{record_id}")
  content_type = "application/json"
  payload = { fields: cell_values }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response

op airtable-record-get(base_id: String, table_id: String, record_id: String) -> Any
  description "Read one record from a table — its cell values are under `fields` in the response, keyed by column name. Returns every column the token can see, in Airtable's default JSON cell format: narrowing or reformatting them needs the `fields[]` and `cellFormat` query parameters this connector cannot encode. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.airtable.com"
  url = fmt("{base}/v0/{base_id}/{table_id}/{record_id}")
  response = http.request(method: "GET", url)
  return response

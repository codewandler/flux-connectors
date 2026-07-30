op airtable-record-delete(base_id: String, table_id: String, record_id: String) -> Any
  description "Delete one record and every cell value in it. There is no API route back: Airtable's trash and revision history are UI features on a retention window, not endpoints, so a flux run cannot undo this. Responds `{\"id\": …, \"deleted\": true}`. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/type` in the response body."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.airtable.com"
  url = fmt("{base}/v0/{base_id}/{table_id}/{record_id}")
  response = http.request(method: "DELETE", url)
  return response

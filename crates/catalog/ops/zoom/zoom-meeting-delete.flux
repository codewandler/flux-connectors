op zoom-meeting-delete(meeting_id: Number) -> Any
  description "Cancel a meeting. It is gone — Zoom offers no undelete, and the meeting id, its join URL and any registrations go with it. Whether Zoom emails the host or the registrants about the cancellation is left to Zoom's own default; this operation cannot ask for either. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.zoom.us"
  url = fmt("{base}/v2/meetings/{meeting_id}")
  response = http.request(method: "DELETE", url)
  return response

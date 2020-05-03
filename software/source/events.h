#ifndef EVENTS_H_
#define EVENTS_H_

#define POWER_EVENT EVENT_MASK(0)
#define SWITCH_EVENT EVENT_MASK(1)
#define PD_ALERT_EVENT EVENT_MASK(2)

extern event_source_t power_event_source;
extern event_source_t switch_event_source;

#endif
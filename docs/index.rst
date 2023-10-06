Reminders formats
=================

.. toctree::
   :maxdepth: 1
   :caption: Contents:


There are four supported types of reminders: one-time, recurring,
countdown and cron-like.

One-time reminders
------------------

The format of a non-recurring reminder for a specific date and time is as follows:
``<date> <time> <description>``, where

-  ``date`` is in either ``day.month.year`` or ``year/month/day``
   formats
-  ``time`` is in the format ``hour:minute``
-  leading zeros in all the fields are optional

Omitting fields
~~~~~~~~~~~~~~~

You can omit the ``year``, ``month``, or the entire ``date`` field. They
will fall back to the nearest possible point in the future.

If you omit the ``minute`` field, it defaults to zero.

Examples
~~~~~~~~

-  ``01.01 0:00 Happy New Year`` => notify on the 1st of January at 12
   AM
-  ``8 wake up`` => notify **today at 8 AM** if it is currently between
   12 AM and 7:59 AM, otherwise notify\ **tomorrow at 8 AM**
-  ``15 13 doctor appointment`` => notify on the nearest 15th day at 1
   PM

Recurring reminders
-------------------

The format for recurring reminders extends the format used for one-time
reminders: ``<date_pattern> <time_pattern> <description>``. Here are the
details:

-  ``date_pattern`` can be specified in either ``date`` or
   ``date_from-date_until/date_divisor`` formats. You can include
   multiple date patterns separated by commas.

   -  ``date_divisor`` can be expressed as ``<years>y<months>m<days>d``
      or ``mon-tue,wed,thu,fri-sat,sun``-like formats

-  ``time_pattern`` can be specified in either ``time`` or
   ``time_from-time_until/time_divisor`` formats (can specify multiple
   with ``,`` separator)

   -  ``time_divisor`` is expressed in the format
      ``<hours>h<minutes>m<seconds>s``.

Omitting fields
~~~~~~~~~~~~~~~

-  ``date_pattern``

   -  Omitting ``date_from`` or ``date_until`` removes the corresponding
      boundary
   -  Omitting ``date_divisor`` defaults to a one-day interval (24
      hours)
   -  Omitting the entire field defaults to the nearest date

-  ``time_pattern``

   -  Omitting ``time_from`` or ``time_until`` removes the corresponding
      boundary

Examples
~~~~~~~~

-  Notify every one and a half hours from 10 AM to 8 PM on weekdays:

   -  ``-/mon-fri 10-20/1h30m take a break``
   -  ``On Monday-Friday at 10-20 every 1hour30mins take a break``

-  Notify on every Sunday from the 1st of April to the 1st of May at
   15:00:

   -  ``1.04-1.05/sun at 15:30 clean the room``
   -  ``01.04-01.05 every Sunday at 15:30 clean the room``

-  Notify on the 20th day of every month at 10 AM:

   -  ``20/1m 10 submit meter readings``

----

Countdown reminders
-------------------

Countdown reminders are set for a specified duration and follow the
``<duration> <description>`` format.

-  ``duration`` is expressed in the format
   ``<years>y<months>mo<weeks>w<days>d<hours>h<minutes>m<seconds>s``

Examples
~~~~~~~~

-  ``5m grab tea`` => notify in 5 minutes
-  ``1d1h`` => notify in 25 hours

Cron-like reminders
-------------------

*NOTE: Originally cron-like reminders were the only way to create a
recurring reminder, but you can still use them.*

Format
~~~~~~

Refer to `cron expression
syntax <https://en.wikipedia.org/wiki/Cron#CRON_expression>`__.

Examples
~~~~~~~~

-  ``55 10 * * 1-5 go to school`` (at 10:55 AM every weekday)
-  ``45 10-19 * * 1-6 break for 15 minutes`` (at 10:45, 11:45, ...,
   19:45 from Monday to Saturday)

Reminders grammar
-----------------

The grammar definitions can be looked up in `the PEST grammar
file <https://github.com/magnickolas/remindee-bot/blob/master/src/grammars/reminder.pest>`__.

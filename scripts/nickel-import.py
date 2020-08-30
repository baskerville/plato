#!/usr/local/env python3

"""Import reading status from Kobo Nickel's database to Plato.

WORK IN PROGRESS. This script really doesn't have enough error checking. This
may corrupt your Plato reading status database.

Currently only supports importing status for .kepub.epub files but may actually
work with .epub files too.
"""

from pathlib import Path
import sqlite3
import json
import urllib.parse
from datetime import datetime
import argparse

parser = argparse.ArgumentParser()
parser.add_argument("--force", help="force replacement of newer reading states", action='store_true')
parser.add_argument("root", help="the path to the mounted Kobo storage")
args = parser.parse_args()

root = Path(args.root)

print(FORCE)
with root.joinpath('.metadata.json').open() as f:
    metadata_json = json.load(f)
    plato_books = {v["file"]["path"]: k for k, v in metadata_json.items()}

connection = sqlite3.connect(root.joinpath('.kobo/KoboReader.sqlite'))
cursor = connection.cursor()
cursor.execute("""
    select
        ContentID,
        Title,
        ChapterIDBookmarked,
        adobe_location,
        ReadStatus,
        ___PercentRead,
        DateLastRead
    from content
    where
        BookID is null;""")
bookmarks = cursor.fetchall()

for ContentID, Title, ChapterIDBookmarked, adobe_location, ReadStatus, PercentRead, DateLastRead in bookmarks:
    if DateLastRead:
        # Timezone is always Zulu (I think)
        # TODO: actually investigate this timestamp
        kobo_last_read = datetime.strptime(DateLastRead, "%Y-%m-%dT%H:%M:%S%z").replace(tzinfo=None)
    else:
        kobo_last_read = None

    parsed_uri = urllib.parse.urlparse(ContentID)

    if parsed_uri.scheme == "file":
        # TODO: handle /mnt/sd too
        book = Path(parsed_uri.path).relative_to('/mnt/onboard')

        if not ChapterIDBookmarked:
            print("SKIPPING book without ChapterIDBookmarked", ContentID)
            continue

        # TODO: support/test with plain .epub
        # TODO: support PDF
        if not book.name.endswith('.kepub.epub'):
            print("SKIPPING non kepub", ContentID)
            continue

        if not book.as_posix() in plato_books:
            print("SKIPPING missing Plato book", ContentID)
            continue

        # Is it always safe to decode the fingerprint id like this?
        fingerprint = f"{int(plato_books[book.as_posix()]):X}"
        # .reading-states should always exist if Plato has been opened
        reading_state_path = root.joinpath(".reading-states", fingerprint + ".json")

        state = {}
        if reading_state_path.exists():
            with reading_state_path.open("r") as f:
                state = json.load(f)
                plato_last_read = datetime.strptime(state["opened"], "%Y-%m-%d %H:%M:%S")
                if(plato_last_read >= kobo_last_read and state["currentPage"] != 0):
                    if not FORCE:
                        print("SKIPPING newer Plato reading_state", ContentID)
                        continue
                    else:
                        print("FORCE UPDATING newer Plato reading_state", ContentID)

        last_read = kobo_last_read or datetime.now()
        state["opened"] = last_read.strftime("%Y-%m-%d %H:%M:%S")
        state["currentUri"] = ChapterIDBookmarked
        state["finished"] = ReadStatus == 2 #TODO: test
        # Fake the page count from the percent read:
        state["currentPage"] = PercentRead or 0
        state["pagesCount"] = 100

        with reading_state_path.open("w") as f:
            json.dump(state, f, indent=2)
    else:
        # An adobe digital editions book
        # TODO: I think adobe_location is used?
        print("SKIPPING Adobe book", ContentID)

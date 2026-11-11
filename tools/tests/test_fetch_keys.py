"""Offline tests for tools/fetch_keys.py; all key material below is synthetic.

    python3 tools/tests/test_fetch_keys.py
"""
import contextlib
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import fetch_keys as fetch  # noqa: E402

TABLE = '''void KeyVault::LoadSelfAPPKeys()
{
\tsk_APP_arr.clear();
\tsk_APP_arr.emplace_back(0x0000, 0x00, 0x0001, KEY_APP, "%s", "%s", "", 0, 0);
\tsk_APP_arr.emplace_back(0x0000, 0x00, 0x0001, KEY_APP, "%s", "%s", "", 0, 0);
\tsk_APP_arr.emplace_back(0x0000, 0x00, 0x0002, KEY_APP, "%s", "%s", "", 0, 0);
}
''' % ('11' * 32, '22' * 16, '33' * 32, '44' * 16, '55' * 32, '66' * 16)


class FetchKeysTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'keys' / 'self_keys.txt'

    def write(self, keys):
        fetch.write_private(str(self.path), fetch.key_file_text(keys, 'synthetic'))

    def test_parse_takes_first_entry_of_a_revision(self):
        keys = fetch.parse_self_keys(TABLE, 'APP')
        self.assertEqual(keys[0], (1, '11' * 32, '22' * 16))
        self.assertEqual([k[0] for k in keys], [1, 2])

    def test_missing_revision_is_an_error(self):
        with self.assertRaises(fetch.FetchError):
            fetch.parse_self_keys(TABLE.replace('0x0001', '0x0003'), 'APP')

    def test_complete_file_and_private_mode(self):
        self.write({'APP': fetch.parse_self_keys(TABLE, 'APP')})
        self.assertTrue(fetch.usable(self.path))
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)

    def test_short_key_is_rejected(self):
        self.write({'APP': [(1, '11' * 31, '22' * 16)]})
        self.assertFalse(fetch.usable(self.path))

    def test_existing_incomplete_file_is_preserved_without_network(self):
        self.path.parent.mkdir()
        self.path.write_text('# keep this file\n')
        with patch.object(fetch, 'download', side_effect=AssertionError('unexpected network')):
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(fetch.main(['--out', str(self.path)]), 1)
        self.assertEqual(self.path.read_text(), '# keep this file\n')

    def test_check_is_read_only_and_offline(self):
        self.write({'APP': fetch.parse_self_keys(TABLE, 'APP')})
        before = self.path.stat().st_mtime_ns
        with patch.object(fetch, 'download', side_effect=AssertionError('unexpected network')):
            with contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(fetch.main(['--out', str(self.path), '--check']), 0)
        self.assertEqual(out.getvalue().strip(), str(self.path))
        self.assertEqual(before, self.path.stat().st_mtime_ns)

    def test_wrong_source_hash_never_overwrites_keys(self):
        self.path.parent.mkdir()
        self.path.write_text('# keep this file\n')
        source = Path(self.tmp.name) / 'key_vault.cpp'
        source.write_text(TABLE)
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(fetch.main(['--out', str(self.path), '--source', str(source), '--force']), 1)
        self.assertEqual(self.path.read_text(), '# keep this file\n')

    def test_default_path_is_outside_the_repository(self):
        with patch.dict(os.environ, {}, clear=True):
            p = fetch.default_path()
            self.assertIn('afro-samurai-recomp', p)
            self.assertFalse(p.startswith(str(Path(fetch.__file__).resolve().parent.parent)))
        with patch.dict(os.environ, {'AFRO_KEYS': '/custom/self_keys.txt'}):
            self.assertEqual(fetch.default_path(), '/custom/self_keys.txt')


if __name__ == '__main__':
    unittest.main()

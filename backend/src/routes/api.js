const express = require('express');
const router = express.Router();

const archiveRoutes = require('./archiveRoutes');
const tagRoutes = require('./tagRoutes');
const categoryRoutes = require('./categoryRoutes');
const historyRoutes = require('./historyRoutes');
const settingsRoutes = require('./settingsRoutes');
const backupRoutes = require('./backupRoutes');

router.get('/', (req, res) => {
  res.json({ message: 'MangaViewer API v2', version: '2.0.0' });
});

router.use('/', archiveRoutes);
router.use('/', tagRoutes);
router.use('/', categoryRoutes);
router.use('/', historyRoutes);
router.use('/', settingsRoutes);
router.use('/', backupRoutes);

module.exports = router;

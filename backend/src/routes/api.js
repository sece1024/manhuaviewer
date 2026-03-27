const express = require('express');
const router = express.Router();

const archiveRoutes = require('./archiveRoutes');
const tagRoutes = require('./tagRoutes');
const categoryRoutes = require('./categoryRoutes');
const historyRoutes = require('./historyRoutes');
const settingsRoutes = require('./settingsRoutes');

router.get('/', (req, res) => {
  res.json({ message: 'MangaViewer API v2', version: '2.0.0' });
});

router.use('/', archiveRoutes);
router.use('/', tagRoutes);
router.use('/', categoryRoutes);
router.use('/', historyRoutes);
router.use('/', settingsRoutes);

module.exports = router;

const express = require('express');
const router = express.Router();

const configRoutes = require('./configRoutes');
const folderRoutes = require('./folderRoutes');
const imageRoutes = require('./imageRoutes');
const historyRoutes = require('./historyRoutes');
const tagRoutes = require('./tagRoutes');
const settingsRoutes = require('./settingsRoutes');

router.get('/', (req, res) => {
  res.json({ message: 'MangaViewer API', version: '0.1.0' });
});

router.use('/', configRoutes);
router.use('/', folderRoutes);
router.use('/', imageRoutes);
router.use('/', historyRoutes);
router.use('/', tagRoutes);
router.use('/', settingsRoutes);

module.exports = router;

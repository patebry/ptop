// temp: dump screen rows after draw
const args = process.argv.slice(2);
process.env.MIRROR_DEBUG = '1';
require('./vtop-mirror.js');

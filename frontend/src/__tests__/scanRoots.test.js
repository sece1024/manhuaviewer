import {
  parseScanRoots,
  addScanRoot,
  removeScanRoot,
  updateScanRootDepth,
  serializeScanRoots,
  clampDepth,
  MAX_SCAN_ROOTS,
} from '../utils/scanRoots';

describe('clampDepth', () => {
  test('钳到 1..5', () => {
    expect(clampDepth(0)).toBe(1);
    expect(clampDepth(9)).toBe(5);
    expect(clampDepth('3')).toBe(3);
  });

  test('非法值用兜底', () => {
    expect(clampDepth('abc', 2)).toBe(2);
    expect(clampDepth(undefined, 2)).toBe(2);
    expect(clampDepth(null)).toBe(1);
  });
});

describe('parseScanRoots', () => {
  test('解析正常列表并钳深度', () => {
    const raw = JSON.stringify([{ path: '/a', depth: 2 }, { path: '/b', depth: 99 }]);
    expect(parseScanRoots(raw)).toEqual([{ path: '/a', depth: 2 }, { path: '/b', depth: 5 }]);
  });

  test('坏 JSON / 非数组 / 缺 path 都不会让扫描区渲染不出来', () => {
    expect(parseScanRoots('{not json')).toEqual([]);
    expect(parseScanRoots('"a string"')).toEqual([]);
    expect(parseScanRoots(JSON.stringify([{ depth: 2 }, null, { path: '  ' }]))).toEqual([]);
    expect(parseScanRoots('')).toEqual([]);
    expect(parseScanRoots(null)).toEqual([]);
  });

  test('列表为空时用旧的单根设置种一条（升级路径不丢扫描目录）', () => {
    expect(parseScanRoots('', '/old/lib', '3')).toEqual([{ path: '/old/lib', depth: 3 }]);
    expect(parseScanRoots('[]', '/old/lib', '2')).toEqual([{ path: '/old/lib', depth: 2 }]);
    // 坏值同样兜到旧设置上
    expect(parseScanRoots('{oops', '/old/lib')).toEqual([{ path: '/old/lib', depth: 1 }]);
  });

  test('没有旧设置也没有列表时返回空（新用户看到引导）', () => {
    expect(parseScanRoots('', '', '1')).toEqual([]);
    expect(parseScanRoots('[]', '   ')).toEqual([]);
  });

  test('列表已有内容时不再叠加旧的单根设置', () => {
    const raw = JSON.stringify([{ path: '/a', depth: 1 }]);
    expect(parseScanRoots(raw, '/old/lib', '2')).toEqual([{ path: '/a', depth: 1 }]);
  });

  test('超出上限时截断', () => {
    const raw = JSON.stringify(Array.from({ length: MAX_SCAN_ROOTS + 3 }, (_, i) => ({ path: `/r${i}`, depth: 1 })));
    expect(parseScanRoots(raw)).toHaveLength(MAX_SCAN_ROOTS);
  });
});

describe('addScanRoot', () => {
  test('新目录插到最前', () => {
    const roots = [{ path: '/a', depth: 1 }];
    expect(addScanRoot(roots, '/b', 2)).toEqual([{ path: '/b', depth: 2 }, { path: '/a', depth: 1 }]);
  });

  test('同一路径只保留一条并更新深度', () => {
    const roots = [{ path: '/a', depth: 1 }, { path: '/b', depth: 1 }];
    expect(addScanRoot(roots, '/b', 4)).toEqual([{ path: '/b', depth: 4 }, { path: '/a', depth: 1 }]);
  });

  test('空路径不产生条目', () => {
    expect(addScanRoot([{ path: '/a', depth: 1 }], '   ')).toEqual([{ path: '/a', depth: 1 }]);
  });

  test('超出上限丢最旧的', () => {
    let roots = [];
    for (let i = 0; i < MAX_SCAN_ROOTS + 2; i += 1) roots = addScanRoot(roots, `/r${i}`, 1);
    expect(roots).toHaveLength(MAX_SCAN_ROOTS);
    expect(roots[0].path).toBe(`/r${MAX_SCAN_ROOTS + 1}`);
    expect(roots.map(r => r.path)).not.toContain('/r0');
  });

  test('路径只 trim，不去尾部分隔符（Windows 的 C:\\ 不能变成 C:）', () => {
    expect(addScanRoot([], '  /a/  ', 1)[0].path).toBe('/a/');
    expect(addScanRoot([], 'C:\\', 1)[0].path).toBe('C:\\');
  });
});

describe('removeScanRoot / updateScanRootDepth / serializeScanRoots', () => {
  const roots = [{ path: '/a', depth: 1 }, { path: '/b', depth: 2 }];

  test('按路径全等移除', () => {
    expect(removeScanRoot(roots, '/a')).toEqual([{ path: '/b', depth: 2 }]);
    expect(removeScanRoot(roots, '/missing')).toEqual(roots);
  });

  test('改深度只影响命中的那条', () => {
    expect(updateScanRootDepth(roots, '/b', 5)).toEqual([{ path: '/a', depth: 1 }, { path: '/b', depth: 5 }]);
  });

  test('序列化后可原样解析回来（往返一致）', () => {
    expect(parseScanRoots(serializeScanRoots(roots))).toEqual(roots);
  });
});

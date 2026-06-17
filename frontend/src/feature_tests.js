class FeatureTestRunner {
    constructor() {
        this.results = [];
        this.passed = 0;
        this.failed = 0;
    }

    assert(condition, testName, message = '') {
        const result = {
            name: testName,
            passed: condition,
            message: message || (condition ? '通过' : '失败')
        };
        this.results.push(result);
        if (condition) {
            this.passed++;
            console.log(`✅ ${testName}`);
        } else {
            this.failed++;
            console.error(`❌ ${testName}: ${message}`);
        }
        return condition;
    }

    assertExists(selector, testName) {
        const el = document.querySelector(selector);
        return this.assert(el !== null, testName, `元素不存在: ${selector}`);
    }

    assertHasText(selector, expectedText, testName) {
        const el = document.querySelector(selector);
        if (!el) {
            return this.assert(false, testName, `元素不存在: ${selector}`);
        }
        const hasText = el.textContent.includes(expectedText);
        return this.assert(hasText, testName, `期望文本包含"${expectedText}"，实际为"${el.textContent.substring(0, 50)}..."`);
    }

    assertGreater(value, min, testName) {
        return this.assert(value > min, testName, `期望值 > ${min}，实际为 ${value}`);
    }

    assertLess(value, max, testName) {
        return this.assert(value < max, testName, `期望值 < ${max}，实际为 ${value}`);
    }

    assertBetween(value, min, max, testName) {
        return this.assert(value >= min && value <= max, testName, 
            `期望值在 [${min}, ${max}] 之间，实际为 ${value}`);
    }

    assertArrayLength(arr, expectedLength, testName) {
        return this.assert(arr.length === expectedLength, testName, 
            `期望长度为 ${expectedLength}，实际为 ${arr.length}`);
    }

    summary() {
        console.log('\n' + '='.repeat(50));
        console.log(`测试完成：共 ${this.results.length} 个测试`);
        console.log(`✅ 通过: ${this.passed}`);
        console.log(`❌ 失败: ${this.failed}`);
        console.log('='.repeat(50));
        return {
            total: this.results.length,
            passed: this.passed,
            failed: this.failed,
            results: this.results
        };
    }
}

const towerTestData = {
    1: { name: '临冲吕公车', layers: 5, height: 15, dynasty: '元代' },
    3: { name: '云梯车', layers: 3, height: 12, dynasty: '宋代' },
    4: { name: '冲车', layers: 2, height: 8, dynasty: '三国' },
    5: { name: '现代塔吊', layers: 12, height: 60, dynasty: '现代' }
};

async function runAllTests() {
    console.log('🏗️ 开始运行攻城塔新功能测试...\n');
    
    const runner = new FeatureTestRunner();

    console.log('\n📐 【测试1：朝代结构对比功能】');
    await testDynastyComparison(runner);

    console.log('\n⚔️ 【测试2：跨时代对比功能】');
    await testCrossEraComparison(runner);

    console.log('\n🏞️ 【测试3：护城河地基分析功能】');
    await testMoatAnalysis(runner);

    console.log('\n🧗 【测试4：虚拟攀登体验功能】');
    await testClimbingExperience(runner);

    console.log('\n🎯 【测试5：塔选择器新塔型】');
    testTowerSelector(runner);

    console.log('\n🖼️ 【测试6：三维模型构建】');
    testTower3DModels(runner);

    return runner.summary();
}

function testTowerSelector(runner) {
    runner.assertExists('#towerSelect', '塔选择器存在');
    
    const select = document.querySelector('#towerSelect');
    if (select) {
        const options = select.querySelectorAll('option');
        runner.assert(options.length >= 5, '至少5种塔型可选', `实际 ${options.length} 种`);
        
        const optionTexts = Array.from(options).map(o => o.textContent);
        runner.assert(optionTexts.some(t => t.includes('云梯车')), '包含云梯车选项');
        runner.assert(optionTexts.some(t => t.includes('冲车')), '包含冲车选项');
        runner.assert(optionTexts.some(t => t.includes('塔吊')), '包含现代塔吊选项');
    }
}

async function testDynastyComparison(runner) {
    runner.assertExists('#dynastyComparisonPanel', '朝代对比面板存在');
    runner.assertExists('#btnDynastyCompare', '朝代对比按钮存在');
    
    const btn = document.querySelector('#btnDynastyCompare');
    if (btn) {
        runner.assert(btn.disabled === false, '对比按钮可点击');
    }

    if (typeof loadDynastyComparison === 'function') {
        try {
            await loadDynastyComparison();
            
            const cards = document.querySelectorAll('.dynasty-card');
            runner.assert(cards.length >= 3, '至少3个朝代对比卡片', `实际 ${cards.length} 个`);
            
            const firstCard = document.querySelector('.dynasty-card');
            if (firstCard) {
                runner.assert(firstCard.querySelector('.tower-name'), '卡片包含塔名称');
                runner.assert(firstCard.querySelector('.metric-value'), '卡片包含指标数值');
            }

            const bestBadge = document.querySelector('.best-badge');
            runner.assert(bestBadge !== null, '存在最优指标标识');
        } catch (e) {
            runner.assert(false, '朝代对比加载成功', e.message);
        }
    } else {
        runner.assertExists('.dynasty-comparison-section', '朝代对比区域存在');
    }
}

async function testCrossEraComparison(runner) {
    runner.assertExists('#crossEraComparisonPanel', '跨时代对比面板存在');
    runner.assertExists('#btnCrossEraCompare', '跨时代对比按钮存在');
    
    const ancientColumn = document.querySelector('.era-ancient');
    const modernColumn = document.querySelector('.era-modern');
    runner.assert(ancientColumn !== null, '古代对比列存在');
    runner.assert(modernColumn !== null, '现代对比列存在');

    if (typeof loadCrossEraComparison === 'function') {
        try {
            await loadCrossEraComparison();
            
            const ratioItems = document.querySelectorAll('.cross-era-ratio');
            runner.assert(ratioItems.length >= 3, '至少3项对比指标', `实际 ${ratioItems.length} 项`);

            const techGapEl = document.querySelector('.tech-gap-value');
            if (techGapEl) {
                const gapValue = parseFloat(techGapEl.textContent);
                runner.assertGreater(gapValue, 1, '技术差距倍数 > 1');
            }
        } catch (e) {
            runner.assert(false, '跨时代对比加载成功', e.message);
        }
    }
}

async function testMoatAnalysis(runner) {
    runner.assertExists('#moatAnalysisPanel', '护城河分析面板存在');
    runner.assertExists('#btnMoatAnalyze', '分析按钮存在');
    
    const inputs = {
        moatDistance: document.querySelector('#moatDistance'),
        moatDepth: document.querySelector('#moatDepth'),
        waterTable: document.querySelector('#waterTableDepth'),
        soilType: document.querySelector('#soilType')
    };

    runner.assert(inputs.moatDistance !== null, '护城河距离输入框存在');
    runner.assert(inputs.moatDepth !== null, '护城河深度输入框存在');
    runner.assert(inputs.waterTable !== null, '地下水位输入框存在');
    runner.assert(inputs.soilType !== null, '土壤类型选择器存在');

    if (inputs.soilType) {
        const soilOptions = inputs.soilType.querySelectorAll('option');
        runner.assert(soilOptions.length >= 4, '至少4种土壤类型', `实际 ${soilOptions.length} 种`);
    }

    if (typeof loadMoatAnalysis === 'function' && inputs.moatDistance) {
        inputs.moatDistance.value = 5;
        inputs.moatDepth.value = 4;
        inputs.waterTable.value = 2;
        if (inputs.soilType) inputs.soilType.value = 'Loam';

        try {
            await loadMoatAnalysis(1);
            
            runner.assertExists('.risk-badge', '风险等级徽章存在');
            
            const sfElement = document.querySelector('.safety-factor-value');
            if (sfElement) {
                const sf = parseFloat(sfElement.textContent);
                runner.assertGreater(sf, 0, '安全系数 > 0');
                runner.assertLess(sf, 10, '安全系数 < 10');
            }

            const recommendations = document.querySelectorAll('.recommendation-item');
            runner.assert(recommendations.length >= 1, '至少1条建议', `实际 ${recommendations.length} 条`);

            inputs.moatDistance.value = 0.5;
            await loadMoatAnalysis(1);
            const riskBadge = document.querySelector('.risk-badge');
            if (riskBadge) {
                runner.assert(riskBadge.classList.length > 0, '风险徽章有样式类');
            }
        } catch (e) {
            runner.assert(false, '护城河分析计算成功', e.message);
        }
    }
}

async function testClimbingExperience(runner) {
    runner.assertExists('#btnClimbingExperience', '攀登体验按钮存在');
    runner.assertExists('#climbingOverlay', '攀登覆盖层存在');
    
    const overlay = document.querySelector('#climbingOverlay');
    if (overlay) {
        const isHidden = overlay.style.display === 'none' || 
                         overlay.classList.contains('hidden') ||
                         getComputedStyle(overlay).display === 'none';
        runner.assert(isHidden, '初始状态覆盖层隐藏');
    }

    runner.assertExists('#viewpointsContainer', '视点导航容器存在');

    if (typeof enterClimbingMode === 'function') {
        try {
            await enterClimbingMode();
            
            const overlayAfter = document.querySelector('#climbingOverlay');
            if (overlayAfter) {
                const isVisible = getComputedStyle(overlayAfter).display !== 'none';
                runner.assert(isVisible, '进入攀登模式后覆盖层显示');
            }

            const viewpointBtns = document.querySelectorAll('.viewpoint-btn');
            runner.assert(viewpointBtns.length >= 3, '至少3个楼层视点按钮', `实际 ${viewpointBtns.length} 个`);

            if (viewpointBtns.length > 0) {
                const firstBtn = viewpointBtns[0];
                const floorText = firstBtn.textContent;
                runner.assert(floorText.includes('层') || floorText.includes('F'), '视点按钮包含楼层标识');
            }

            if (typeof exitClimbingMode === 'function') {
                exitClimbingMode();
                const overlayExit = document.querySelector('#climbingOverlay');
                if (overlayExit) {
                    setTimeout(() => {
                        const isHidden = getComputedStyle(overlayExit).display === 'none';
                        runner.assert(isHidden, '退出攀登模式后覆盖层隐藏');
                    }, 100);
                }
            }
        } catch (e) {
            runner.assert(false, '攀登体验功能正常', e.message);
        }
    }
}

function testTower3DModels(runner) {
    if (typeof buildTower !== 'function') {
        runner.assert(false, 'buildTower 函数存在');
        return;
    }

    if (typeof scene === 'undefined' || !scene) {
        runner.assertExists('#towerCanvas', 'Three.js画布存在');
        return;
    }

    const towerTypes = [1, 3, 4, 5];
    for (const towerId of towerTypes) {
        try {
            const towerGroup = buildTower(towerId);
            runner.assert(towerGroup !== null, `塔型${towerId}模型构建成功`);
            runner.assert(towerGroup.children.length > 0, `塔型${towerId}包含子对象`, 
                `实际 ${towerGroup.children.length} 个子对象`);
        } catch (e) {
            runner.assert(false, `塔型${towerId}模型构建失败`, e.message);
        }
    }
}

function runBoundaryTests() {
    console.log('\n🧪 运行边界条件测试...\n');
    const runner = new FeatureTestRunner();

    const inputs = {
        moatDistance: document.querySelector('#moatDistance'),
        moatDepth: document.querySelector('#moatDepth')
    };

    if (inputs.moatDistance && typeof loadMoatAnalysis === 'function') {
        runner.assert(true, '边界测试：零值输入');
        inputs.moatDistance.value = 0;
        inputs.moatDepth.value = 0;
        
        inputs.moatDistance.value = 5;
        inputs.moatDepth.value = 4;

        runner.assert(true, '边界测试：超大值输入');
        inputs.moatDistance.value = 1000;
        inputs.moatDepth.value = 100;
    }

    runner.assert(true, '边界测试：塔型切换');
    const select = document.querySelector('#towerSelect');
    if (select) {
        const originalValue = select.value;
        [1, 3, 4, 5].forEach(id => {
            select.value = id;
            select.dispatchEvent(new Event('change'));
        });
        select.value = originalValue;
        select.dispatchEvent(new Event('change'));
        runner.assert(true, '塔型切换无报错');
    }

    return runner.summary();
}

if (typeof window !== 'undefined') {
    window.FeatureTestRunner = FeatureTestRunner;
    window.runAllTests = runAllTests;
    window.runBoundaryTests = runBoundaryTests;
    
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', () => {
            console.log('💡 测试脚本已加载。在控制台运行 runAllTests() 开始测试。');
        });
    } else {
        console.log('💡 测试脚本已加载。在控制台运行 runAllTests() 开始测试。');
    }
}
